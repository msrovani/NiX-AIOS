//! M9 — skills wasmi: módulo content-addressed + registo no CRDT + execução
//! frugal (fuel 100k, heap 256 KiB, **sem capacidades** por omissão).
//!
//! `skill.install` aceita `wat` (texto, compilado no servidor) ou `wasm_b64`.
//! O hash do módulo é FNV-1a64 (id de conteúdo; a integridade forte virá com
//! BLAKE3 no M8.1). A skill activa é `skill/<nome>/active` no CRDT — trocar de
//! versão é uma op assinada, rollback é apontar para o hash anterior.

use super::*;
use nos_ipc::{Request, Response};
use serde_json::json;

pub fn skills_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("skills")
}

/// S1: commitment BLAKE3 (64 hex). FNV ficou como fingerprint, não integridade.
fn content_id(bytes: &[u8]) -> String {
    nos_mesh::content_hash(bytes)
}

/// S1: manifesto mínimo por skill (vive em `skill/<nome>/meta`).
#[allow(clippy::too_many_arguments)] // manifesto: campos explicitos > struct cerrada
pub fn put_meta(
    state: &AppState,
    name: &str,
    focus: &str,
    version: &str,
    hash: &str,
    bytes: usize,
    provenance: &str,
    caps: &[String],
) {
    let focus = if focus.is_empty() { name } else { focus };
    let version = if version.is_empty() { "0.1.0" } else { version };
    let meta = json!({
        "name": name,
        "focus": focus,
        "version": version,
        "caps": caps,
        "hash": hash,
        "bytes": bytes,
        "provenance": provenance,
        "published_at_ms": 0,
    });
    state.mesh.crdt_put(
        state.mesh.local_id(),
        &format!("skill/{name}/meta"),
        &meta.to_string(),
        0,
    );
}

/// S2: mensagem can?nica assinada pelo publicador.
pub fn publisher_msg(name: &str, focus: &str, version: &str, hash: &str) -> String {
    format!("{name}|{focus}|{version}|{hash}")
}

/// S2: allowlist de publishers (hex), via `NOSD_SKILL_PUBLISHERS`.
fn trusted_publishers() -> Vec<String> {
    std::env::var("NOSD_SKILL_PUBLISHERS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// S2: `Ok("signed")` se assinatura v?lida e publisher autorizado;
/// `Ok("local")` se n?o h? assinatura (install local, sem autor);
/// `Err` se a assinatura for inv?lida ou o publisher n?o estiver na allowlist.
pub fn verify_publisher(
    name: &str,
    focus: &str,
    version: &str,
    hash: &str,
    publisher_hex: Option<&str>,
    sig_b64: Option<&str>,
) -> Result<&'static str, String> {
    use base64::Engine as _;
    let (Some(pk_hex), Some(sig_b64)) = (publisher_hex, sig_b64) else {
        return Ok("local");
    };
    let pk = hex_decode(pk_hex).ok_or("publisher hex invalido")?;
    if pk.len() != 32 {
        return Err("publisher tem de ter 32 bytes".into());
    }
    let allow = trusted_publishers();
    if !allow.is_empty() && !allow.iter().any(|a| a == &pk_hex.to_ascii_lowercase()) {
        return Err("publisher fora da allowlist (NOSD_SKILL_PUBLISHERS)".into());
    }
    let sig = base64::engine::general_purpose::STANDARD
        .decode(sig_b64)
        .map_err(|e| format!("sig base64: {e}"))?;
    let msg = publisher_msg(name, focus, version, hash);
    if !nos_mesh::ident::verify(&pk, msg.as_bytes(), &sig) {
        return Err("assinatura do manifesto invalida".into());
    }
    Ok("signed")
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// S2b: capacidades que o host pode expor a uma skill. Qualquer outra = recusa.
pub const ALLOWED_CAPS: &[&str] = &["aios_ai.infer", "log"];

pub fn verify_caps(caps: &[String]) -> Result<(), String> {
    for c in caps {
        if !ALLOWED_CAPS.contains(&c.as_str()) {
            return Err(format!(
                "cap '{c}' nao permitida (permitidas: {ALLOWED_CAPS:?})"
            ));
        }
    }
    Ok(())
}

fn max_version_for_focus(state: &AppState, focus: &str) -> Option<String> {
    candidate_metas(state, focus)
        .iter()
        .filter_map(|m| m["version"].as_str().map(|v| v.to_string()))
        .max_by_key(|v| ver_key(v))
}

fn ver_key(v: &str) -> (u32, u32, u32) {
    let mut it = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// S1: score 0-100 + breakdown auditável (proveniência, caps, versão, tamanho).
fn score_meta(
    m: &serde_json::Value,
    all: &[serde_json::Value],
    stats: &std::collections::HashMap<String, (u64, u64)>,
) -> (u32, serde_json::Value) {
    let prov = match m["provenance"].as_str().unwrap_or("local") {
        "signed" => 30,
        "peer" => 18,
        _ => 10,
    };
    let n_caps = m["caps"].as_array().map(|a| a.len()).unwrap_or(0) as u32;
    let caps = 20u32.saturating_sub(4 * n_caps);
    let vk = ver_key(m["version"].as_str().unwrap_or("0.0.0"));
    let max_vk = all
        .iter()
        .map(|x| ver_key(x["version"].as_str().unwrap_or("0.0.0")))
        .max()
        .unwrap_or((0, 0, 0));
    let version = if vk == max_vk { 40 } else { 20 };
    let bytes = m["bytes"].as_u64().unwrap_or(u64::MAX);
    let min_bytes = all
        .iter()
        .filter_map(|x| x["bytes"].as_u64())
        .min()
        .unwrap_or(bytes);
    let size = if bytes <= min_bytes { 10 } else { 5 };
    // S2c: historico local (sucesso/falha observados por este no).
    let (ok, fail) = stats
        .get(m["name"].as_str().unwrap_or(""))
        .copied()
        .unwrap_or((0, 0));
    let runs = ok + fail;
    let history = if runs >= 5 {
        (20 * ok / runs) as u32
    } else {
        8 // sem amostra suficiente: neutro-baixo, nao premeia nem pune
    };
    let total = prov + caps + version + size + history;
    (
        total,
        json!({
            "provenance": prov,
            "caps": caps,
            "version": version,
            "size": size,
            "history": history,
            "runs": [ok, fail],
        }),
    )
}

/// S2c: contadores por skill (`skill/<nome>/stats` = [ok, fail]).
fn bump_stats(state: &AppState, name: &str, ok: bool) {
    let key = format!("skill/{name}/stats");
    let (o, f) = state
        .mesh
        .crdt_get(&key)
        .and_then(|e| serde_json::from_str::<(u64, u64)>(&e.value).ok())
        .unwrap_or((0, 0));
    let (o, f) = if ok { (o + 1, f) } else { (o, f + 1) };
    state
        .mesh
        .crdt_put(state.mesh.local_id(), &key, &json!([o, f]).to_string(), 0);
}

fn candidate_metas(state: &AppState, focus: &str) -> Vec<serde_json::Value> {
    let store = state.mesh.crdt_snapshot();
    let mut out: Vec<serde_json::Value> = store
        .entries
        .iter()
        .filter_map(|(k, e)| {
            if !(k.starts_with("skill/") && k.ends_with("/meta")) {
                return None;
            }
            let m: serde_json::Value = serde_json::from_str(&e.value).ok()?;
            if m["focus"].as_str().unwrap_or("") != focus {
                return None;
            }
            Some(m)
        })
        .collect();
    out.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });
    out
}

fn err(id: u64, msg: impl Into<String>) -> Response {
    Response {
        id,
        ok: false,
        error: Some(msg.into()),
        result: json!(null),
    }
}

/// Instala/activa um módulo. `expect_hash` (M9.1) obriga a que o hash do
/// conteúdo recebido bata com o anunciado — senão recusa (fail-closed).
pub fn install_bytes(
    state: &AppState,
    name: &str,
    bytes: &[u8],
    expect_hash: Option<&str>,
) -> Result<(String, usize), String> {
    if name.is_empty() || name.contains('/') || name.len() > 64 {
        return Err("nome invalido".into());
    }
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("modulo acima de 4 MiB".into());
    }
    let cid = content_id(bytes);
    if let Some(expect) = expect_hash {
        if expect != cid {
            return Err(format!("hash nao bate (esperado {expect}, recebido {cid})"));
        }
    }
    // Valida antes de gravar (Fuel nao e erro de modulo: pode ser intencional).
    if let Err(e) = nos_wasm::FrugalRuntime::new().run_i32(bytes, nos_core::CapBits::NONE, 1, 0) {
        if !matches!(e, nos_wasm::WasmError::Fuel) {
            return Err(format!("modulo invalido: {e}"));
        }
    }
    let dir = skills_dir(state);
    std::fs::create_dir_all(&dir).map_err(|e| format!("skills dir: {e}"))?;
    std::fs::write(dir.join(format!("{cid}.wasm")), bytes).map_err(|e| format!("write: {e}"))?;
    state.mesh.crdt_put(
        state.mesh.local_id(),
        &format!("skill/{name}/active"),
        &cid,
        0,
    );
    Ok((cid, bytes.len()))
}

pub(crate) fn dispatch(state: &AppState, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "skill.install" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if name.is_empty() || name.contains('/') || name.len() > 64 {
                return Some(err(req.id, "skill.install: nome invalido"));
            }
            let bytes: Vec<u8> = if let Some(wat) = req.params.get("wat").and_then(|v| v.as_str()) {
                match wat::parse_str(wat) {
                    Ok(b) => b,
                    Err(e) => return Some(err(req.id, format!("wat: {e}"))),
                }
            } else if let Some(b64) = req.params.get("wasm_b64").and_then(|v| v.as_str()) {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                    Ok(b) => b,
                    Err(e) => return Some(err(req.id, format!("base64: {e}"))),
                }
            } else {
                return Some(err(req.id, "skill.install requer wat ou wasm_b64"));
            };
            if bytes.len() > 4 * 1024 * 1024 {
                return Some(err(req.id, "modulo acima de 4 MiB"));
            }
            // Valida antes de gravar (fail-closed).
            if let Err(e) =
                nos_wasm::FrugalRuntime::new().run_i32(&bytes, nos_core::CapBits::NONE, 1, 0)
            {
                if !matches!(e, nos_wasm::WasmError::Fuel) {
                    return Some(err(req.id, format!("modulo invalido: {e}")));
                }
            }
            let dir = skills_dir(state);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return Some(err(req.id, format!("skills dir: {e}")));
            }
            let cid_pre = content_id(&bytes);
            let focus_p = req
                .params
                .get("focus")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let version_p = req
                .params
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let focus_e = if focus_p.is_empty() { name } else { focus_p };
            let version_e = if version_p.is_empty() {
                "0.1.0"
            } else {
                version_p
            };
            // S2: autoria ANTES de escrever (fail-closed).
            let provenance = match verify_publisher(
                name,
                focus_e,
                version_e,
                &cid_pre,
                req.params.get("publisher").and_then(|v| v.as_str()),
                req.params.get("sig").and_then(|v| v.as_str()),
            ) {
                Ok(p) => p,
                Err(e) => return Some(err(req.id, format!("publisher: {e}"))),
            };
            // S2b: capacidades declaradas (allowlist fechada).
            let caps: Vec<String> = req
                .params
                .get("caps")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if let Err(e) = verify_caps(&caps) {
                return Some(err(req.id, format!("caps: {e}")));
            }
            // S2b: versao monot?nica por focus (sem downgrade silencioso).
            if let Some(maxv) = max_version_for_focus(state, focus_e) {
                if ver_key(version_e) < ver_key(&maxv) {
                    return Some(err(
                        req.id,
                        format!("downgrade recusado: {version_e} < {maxv} (focus {focus_e})"),
                    ));
                }
            }
            let cid = content_id(&bytes);
            let path = dir.join(format!("{cid}.wasm"));
            if let Err(e) = std::fs::write(&path, &bytes) {
                return Some(err(req.id, format!("write: {e}")));
            }
            state.mesh.crdt_put(
                state.mesh.local_id(),
                &format!("skill/{name}/active"),
                &cid,
                0,
            );
            put_meta(
                state,
                name,
                focus_e,
                version_e,
                &cid,
                bytes.len(),
                provenance,
                &caps,
            );
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "name": name,
                    "hash": cid,
                    "bytes": bytes.len(),
                    "focus": focus_e,
                    "version": version_e,
                    "provenance": provenance,
                    "caps": caps,
                    "path": path.display().to_string(),
                }),
            }
        }
        "skill.list" => {
            let store = state.mesh.crdt_snapshot();
            let items: Vec<serde_json::Value> = store
                .entries
                .iter()
                .filter_map(|(k, e)| {
                    k.strip_prefix("skill/")
                        .and_then(|r| r.strip_suffix("/active"))
                        .map(|name| json!({ "name": name, "hash": e.value }))
                })
                .collect();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!(items),
            }
        }
        "skill.run" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arg = req.params.get("arg").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let key = format!("skill/{name}/active");
            let Some(entry) = state.mesh.crdt_get(&key) else {
                return Some(err(req.id, format!("skill '{name}' nao instalada")));
            };
            let path = skills_dir(state).join(format!("{}.wasm", entry.value));
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return Some(err(req.id, format!("modulo {}: {e}", entry.value))),
            };
            // Sem capacidades por omissão: `aios_ai::infer` devolve -1.
            let rt = nos_wasm::FrugalRuntime::new();
            let outcome = rt.run_i32(&bytes, nos_core::CapBits::NONE, 1, arg);
            // S2c: historico local (alimenta o score do Hermes).
            bump_stats(state, name, outcome.is_ok());
            match outcome {
                Ok(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "name": name, "arg": arg, "value": v, "hash": entry.value }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "name": name, "hash": entry.value }),
                },
            }
        }
        "skill.explain" | "skill.select" => {
            let focus = req
                .params
                .get("focus")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if focus.is_empty() {
                return Some(err(req.id, "skill.explain requer focus"));
            }
            let snapshot = state.mesh.crdt_snapshot();
            let mut stats: std::collections::HashMap<String, (u64, u64)> =
                std::collections::HashMap::new();
            for (k, e) in &snapshot.entries {
                if let Some(n) = k
                    .strip_prefix("skill/")
                    .and_then(|r| r.strip_suffix("/stats"))
                {
                    if let Ok(v) = serde_json::from_str::<(u64, u64)>(&e.value) {
                        stats.insert(n.to_string(), v);
                    }
                }
            }
            let metas = candidate_metas(state, focus);
            if metas.is_empty() {
                return Some(err(req.id, format!("sem candidatas para focus '{focus}'")));
            }
            let mut best: Option<(u32, serde_json::Value, serde_json::Value)> = None;
            for m in &metas {
                let (s, why) = score_meta(m, &metas, &stats);
                let better = match &best {
                    None => true,
                    Some((bs, bm, _)) => {
                        s > *bs
                            || (s == *bs
                                && ver_key(m["version"].as_str().unwrap_or("0.0.0"))
                                    > ver_key(bm["version"].as_str().unwrap_or("0.0.0")))
                    }
                };
                if better {
                    best = Some((s, m.clone(), why));
                }
            }
            let (score, chosen, why) = best.expect("nao vazio");
            let name = chosen["name"].as_str().unwrap_or("");
            let version = chosen["version"].as_str().unwrap_or("0.0.0");
            state.mesh.crdt_put(
                state.mesh.local_id(),
                &format!("focus/{focus}/active"),
                &format!("{name}@{version}"),
                0,
            );
            state.mesh.crdt_put(
                state.mesh.local_id(),
                &format!("focus/{focus}/why"),
                &why.to_string(),
                0,
            );
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "focus": focus,
                    "chosen": format!("{name}@{version}"),
                    "score": score,
                    "why": why,
                    "candidates": metas,
                }),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod s2_tests {
    use super::*;
    use nos_mesh::ident::Identity;

    #[test]
    fn signed_manifest_is_accepted_and_tamper_rejected() {
        use base64::Engine as _;
        let id = Identity::from_seed([91u8; 32], Default::default(), true);
        let pk = id.public_key();
        let pk_hex: String = pk.iter().map(|b| format!("{b:02x}")).collect();
        let hash = "a".repeat(64);
        let msg = publisher_msg("sum", "sum", "1.0.0", &hash);
        let sig = id.sign(msg.as_bytes());
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig);

        assert_eq!(
            verify_publisher("sum", "sum", "1.0.0", &hash, Some(&pk_hex), Some(&sig_b64)),
            Ok("signed")
        );
        // Manifesto adulterado (versao diferente) deixa de bater.
        let bad = verify_publisher("sum", "sum", "2.0.0", &hash, Some(&pk_hex), Some(&sig_b64));
        assert!(bad.is_err(), "{bad:?}");
        // Sem assinatura = local (nao e' "signed").
        assert_eq!(
            verify_publisher("sum", "sum", "1.0.0", &hash, None, None),
            Ok("local")
        );
    }
}

#[cfg(test)]
mod s2b_tests {
    use super::*;

    #[test]
    fn declared_caps_are_restricted_to_allowlist() {
        assert!(verify_caps(&[]).is_ok());
        assert!(verify_caps(&["aios_ai.infer".to_string(), "log".to_string()]).is_ok());
        let bad = verify_caps(&["fs.read".to_string()]);
        assert!(bad.is_err(), "{bad:?}");
        let bad2 = verify_caps(&["net.connect".to_string()]);
        assert!(bad2.is_err(), "{bad2:?}");
    }

    #[test]
    fn semver_key_orders_versions() {
        assert!(ver_key("2.0.0") > ver_key("1.9.9"));
        assert!(ver_key("1.10.0") > ver_key("1.9.0"));
        assert_eq!(ver_key("1.2"), ver_key("1.2.0"));
    }
}
