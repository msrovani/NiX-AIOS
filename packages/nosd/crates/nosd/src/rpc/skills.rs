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
pub fn put_meta(
    state: &AppState,
    name: &str,
    focus: &str,
    version: &str,
    hash: &str,
    bytes: usize,
    provenance: &str,
) {
    let focus = if focus.is_empty() { name } else { focus };
    let version = if version.is_empty() { "0.1.0" } else { version };
    let meta = json!({
        "name": name,
        "focus": focus,
        "version": version,
        "caps": [],
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

fn ver_key(v: &str) -> (u32, u32, u32) {
    let mut it = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// S1: score 0-100 + breakdown auditável (proveniência, caps, versão, tamanho).
fn score_meta(m: &serde_json::Value, all: &[serde_json::Value]) -> (u32, serde_json::Value) {
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
    let total = prov + caps + version + size;
    (
        total,
        json!({
            "provenance": prov,
            "caps": caps,
            "version": version,
            "size": size,
        }),
    )
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
            let focus = req
                .params
                .get("focus")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let version = req
                .params
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            put_meta(state, name, focus, version, &cid, bytes.len(), "local");
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "name": name,
                    "hash": cid,
                    "bytes": bytes.len(),
                    "focus": if focus.is_empty() { name } else { focus },
                    "version": if version.is_empty() { "0.1.0" } else { version },
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
            match rt.run_i32(&bytes, nos_core::CapBits::NONE, 1, arg) {
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
            let metas = candidate_metas(state, focus);
            if metas.is_empty() {
                return Some(err(req.id, format!("sem candidatas para focus '{focus}'")));
            }
            let mut best: Option<(u32, serde_json::Value, serde_json::Value)> = None;
            for m in &metas {
                let (s, why) = score_meta(m, &metas);
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
