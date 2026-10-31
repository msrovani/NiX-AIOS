//! M9 — skills wasmi: módulo content-addressed + registo no CRDT + execução
//! frugal (fuel 100k, heap 256 KiB) com **caps declaradas e concedidas por
//! política do host**, não por confiança no manifesto.
//!
//! Um só caminho de instalação (`install`) serve `skill.install` e o hook do
//! `skill.push`: valida nome/hash, import allowlist, assinatura de publisher,
//! caps, downgrade e só depois grava + activa. `skill.run` lê o manifesto,
//! intersecciona as caps declaradas com `NOSD_SKILL_GRANT_CAPS` (default
//! `log`; `aios_ai.infer` exige opt-in) e devolve fuel medido, logs (ABI v2) e
//! o buffer `result_ptr`/`result_len` quando existe.

use super::*;
use nos_core::CapBits;
use nos_ipc::{Request, Response};
use nos_wasm::Grants;
use serde_json::json;
use std::collections::HashMap;

pub fn skills_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("skills")
}

/// S1: commitment BLAKE3 (64 hex). FNV ficou como fingerprint, não integridade.
fn content_id(bytes: &[u8]) -> String {
    nos_mesh::content_hash(bytes)
}

/// S1: manifesto por skill (vive em `skill/<nome>/meta`). Campos explícitos
/// em vez de 8 parâmetros soltos: `kind`/`abi` deixam de ser adivinhados pelo
/// hash e `publisher`/`sig` viajam no próprio manifesto (S2c).
pub struct SkillMeta<'a> {
    pub name: &'a str,
    pub focus: &'a str,
    pub version: &'a str,
    pub hash: &'a str,
    pub bytes: usize,
    pub provenance: &'a str,
    pub caps: &'a [String],
    pub kind: &'a str,
    pub abi: Option<u32>,
    /// Perfil pedido pelo manifesto (`frugal`|`app`); o host resolve-o em
    /// `nos-wasm` conforme `NOSD_SKILL_APP_PROFILE`.
    pub profile: &'a str,
    pub publisher: Option<&'a str>,
    pub sig: Option<&'a str>,
}

/// S1: grava o manifesto. `published_at_ms` é o instante observado por este
/// nó (relógio local, não autoridade de ordem — a ordem vem do VV do CRDT).
pub fn put_meta(state: &AppState, m: &SkillMeta<'_>) {
    let focus = if m.focus.is_empty() { m.name } else { m.focus };
    let version = if m.version.is_empty() {
        "0.1.0"
    } else {
        m.version
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut meta = json!({
        "name": m.name,
        "focus": focus,
        "version": version,
        "caps": m.caps,
        "hash": m.hash,
        "bytes": m.bytes,
        "provenance": m.provenance,
        "kind": m.kind,
        "published_at_ms": now_ms,
    });
    if let Some(abi) = m.abi {
        meta["abi"] = json!(abi);
    }
    if !m.profile.is_empty() {
        meta["profile"] = json!(m.profile);
    }
    if let Some(p) = m.publisher {
        meta["publisher"] = json!(p);
    }
    if let Some(s) = m.sig {
        meta["sig"] = json!(s);
    }
    state.mesh.crdt_put(
        state.mesh.local_id(),
        &format!("skill/{}/meta", m.name),
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
    revoked: &std::collections::HashSet<String>,
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
    if revoked.contains(&pk_hex.to_ascii_lowercase()) {
        return Err("publisher revogado".into());
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

/// S2c: publishers revogados (`skill/revoked/<hex>` = "1").
pub fn revoked_set(state: &AppState) -> std::collections::HashSet<String> {
    let store = state.mesh.crdt_snapshot();
    store
        .entries
        .keys()
        .filter_map(|k| k.strip_prefix("skill/revoked/").map(|s| s.to_string()))
        .collect()
}

/// S2c: junta publisher/sig ao manifesto (o push reencaminha-os para que o
/// destino possa verificar autoria, em vez de aceitar `peer` sem autor).
pub fn meta_with_auth(
    mut meta: serde_json::Value,
    publisher: Option<&str>,
    sig: Option<&str>,
) -> serde_json::Value {
    if let Some(p) = publisher {
        meta["publisher"] = json!(p);
    }
    if let Some(s) = sig {
        meta["sig"] = json!(s);
    }
    meta
}

/// S2c: payload de push com autoria (se o manifesto a tiver).
pub fn push_payload(
    meta: Option<&serde_json::Value>,
    name: &str,
    hash: &str,
    wasm_b64: &str,
) -> serde_json::Value {
    let mut v = json!({ "name": name, "hash": hash, "wasm_b64": wasm_b64 });
    if let Some(m) = meta {
        for k in ["focus", "version", "publisher", "sig", "profile"] {
            if let Some(x) = m.get(k).and_then(|v| v.as_str()) {
                if !x.is_empty() {
                    v[k] = json!(x);
                }
            }
        }
        // S2b: as caps declaradas viajam com o módulo; o destino volta a
        // validá-las na allowlist e a intersectá-las com a sua própria política.
        if let Some(caps) = m.get("caps") {
            v["caps"] = caps.clone();
        }
        if let Some(kind) = m.get("kind") {
            v["kind"] = kind.clone();
        }
    }
    v
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
    stats: &HashMap<String, (u64, u64)>,
    fuel: &HashMap<String, FuelStat>,
) -> (u32, serde_json::Value) {
    // Pesos somam 100: proveniência 25 + caps 15 + versão 30 + tamanho 5 +
    // histórico 15 + eficiência de fuel 10. O clamp fica como cinto e tirantes.
    let prov = match m["provenance"].as_str().unwrap_or("local") {
        "signed" => 25,
        "peer" => 15,
        _ => 8,
    };
    let n_caps = m["caps"].as_array().map(|a| a.len()).unwrap_or(0) as u32;
    let caps = 15u32.saturating_sub(3 * n_caps);
    let vk = ver_key(m["version"].as_str().unwrap_or("0.0.0"));
    let max_vk = all
        .iter()
        .map(|x| ver_key(x["version"].as_str().unwrap_or("0.0.0")))
        .max()
        .unwrap_or((0, 0, 0));
    let version = if vk == max_vk { 30 } else { 15 };
    let bytes = m["bytes"].as_u64().unwrap_or(u64::MAX);
    let min_bytes = all
        .iter()
        .filter_map(|x| x["bytes"].as_u64())
        .min()
        .unwrap_or(bytes);
    let size = if bytes <= min_bytes { 5 } else { 2 };
    // S2c: historico local (sucesso/falha observados por este no).
    let (ok, fail) = stats
        .get(m["name"].as_str().unwrap_or(""))
        .copied()
        .unwrap_or((0, 0));
    let runs = ok + fail;
    let history = if runs >= 5 {
        (15 * ok / runs) as u32
    } else {
        6 // sem amostra suficiente: neutro-baixo, nao premeia nem pune
    };
    // Eficiência: alvo = 10% do limite do perfil; abaixo do alvo = 10 pontos.
    // Amostra < 3 execuções = 5 (neutro); não se premeia com uma execução só.
    let f = fuel.get(m["name"].as_str().unwrap_or("")).copied();
    let (fuel_points, fuel_avg, fuel_target) = match f {
        Some(f) if f.runs >= 3 && f.limit > 0 => {
            let avg = f.total / f.runs;
            let target = (f.limit / 10).max(1);
            let points = if avg == 0 {
                10
            } else {
                target.saturating_mul(10).saturating_div(avg).min(10) as u32
            };
            (points, avg, target)
        }
        _ => (5, 0, 0),
    };
    let total = (prov + caps + version + size + history + fuel_points).min(100);
    (
        total,
        json!({
            "provenance": prov,
            "caps": caps,
            "version": version,
            "size": size,
            "history": history,
            "runs": [ok, fail],
            "fuel": fuel_points,
            "fuel_avg": fuel_avg,
            "fuel_target": fuel_target,
            "fuel_runs": f.map(|x| x.runs).unwrap_or(0),
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

/// S2c/v2: acumula fuel observado por skill (`skill/<nome>/fuel`).
fn bump_fuel(state: &AppState, name: &str, used: u64, limit: u64) {
    let key = format!("skill/{name}/fuel");
    let mut st: FuelStat = state
        .mesh
        .crdt_get(&key)
        .and_then(|e| serde_json::from_str(&e.value).ok())
        .unwrap_or_default();
    st.runs = st.runs.saturating_add(1);
    st.total = st.total.saturating_add(used);
    st.max = st.max.max(used);
    st.limit = limit;
    if let Ok(v) = serde_json::to_string(&st) {
        state.mesh.crdt_put(state.mesh.local_id(), &key, &v, 0);
    }
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

/// Política do host: só caps listadas aqui podem ser concedidas à execução.
/// Default `log` (canal limitado, sem efeitos); `aios_ai.infer` exige opt-in.
pub const HOST_GRANT_ENV: &str = "NOSD_SKILL_GRANT_CAPS";

/// Opt-in do operador para o perfil `app` (1 MiB / 500k fuel). O manifesto
/// pede; o host decide — um módulo não aumenta o próprio orçamento.
pub const APP_PROFILE_ENV: &str = "NOSD_SKILL_APP_PROFILE";

fn app_profile_enabled() -> bool {
    std::env::var(APP_PROFILE_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Runtime efetivo e perfil aplicado (o pedido nunca é autoridade).
fn runtime_for(requested: &str) -> (&'static nos_wasm::FrugalRuntime, &'static str) {
    if requested == "app" && app_profile_enabled() {
        (nos_wasm::FrugalRuntime::shared_app(), "app")
    } else {
        (nos_wasm::FrugalRuntime::shared(), "frugal")
    }
}

/// Caps efectivas de uma execução: declaradas no manifesto ∩ política do host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillGrants {
    pub bits: CapBits,
    pub log: bool,
    pub names: Vec<String>,
}

/// Janela deslizante de execução por peer autenticado.
///
/// Não é CRDT (a quota não se replica nem se herda por gossip), mas é
/// **persistida** num ficheiro local: sem isso, um restart do `nosd` devolvia
/// orçamento inteiro a um peer que já o tinha esgotado. O tempo é de relógio de
/// parede (epoch ms) porque `Instant` não sobrevive ao processo; um relógio que
/// recue só estende a janela (nega mais), um que avance expira-a mais cedo.
pub struct SkillQuota {
    window: std::time::Duration,
    max_runs: u32,
    max_fuel: u64,
    entries: HashMap<String, QuotaWindow>,
    /// Ficheiro da janela; `None` = só memória (testes de unidade).
    path: Option<std::path::PathBuf>,
    /// Âncora assinada ao lado da janela (anti-rollback + anti-apagamento).
    anchor_path: Option<std::path::PathBuf>,
    /// Chave do nó: com identidade, a janela é assinada e uma assinatura
    /// inválida/ausente é recusa — apagar ou editar o ficheiro não repõe
    /// orçamento. Sem identidade (lab), aceita-se sem assinatura e diz-se isso
    /// no relatório (`signed:false`).
    identity: Option<nos_mesh::Identity>,
    /// Motivo de recusa global (ficheiro ilegível, assinatura inválida,
    /// rollback ou não gravável). Fail-closed: enquanto existir, nenhuma skill
    /// corre — um ficheiro truncado não pode valer um orçamento novo.
    deny: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct QuotaWindow {
    /// Início da janela em epoch ms.
    since_ms: u64,
    runs: u32,
    fuel: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct QuotaFile {
    version: u32,
    window_secs: u64,
    peers: HashMap<String, QuotaWindow>,
    /// `node_id` de quem assinou (ausente em modo lab sem identidade).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node: Option<u32>,
    /// Ed25519 (hex) sobre `canonical()`. Ausente = lab sem identidade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sig: Option<String>,
}

const QUOTA_FILE_VERSION: u32 = 2;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Bytes assinados: ordem **canónica** (peers ordenados por nome), para a
/// assinatura não depender da ordem do `HashMap` no JSON.
fn quota_canonical(window_secs: u64, peers: &HashMap<String, QuotaWindow>) -> Vec<u8> {
    let mut keys: Vec<&String> = peers.keys().collect();
    keys.sort();
    let mut out = format!("NSGQ1\n{window_secs}\n");
    for k in keys {
        let w = peers[k];
        out.push_str(&format!("{k}\t{}\t{}\t{}\n", w.since_ms, w.runs, w.fuel));
    }
    out.into_bytes()
}

fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    for i in (0..b.len()).step_by(2) {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}

/// Escrita atómica: tmp + fsync + rename + fsync do diretório, 0600.
fn write_atomic(path: &std::path::Path, body: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    if let Some(dir) = path.parent() {
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

impl SkillQuota {
    pub fn new(window_secs: u64, max_runs: u32, max_fuel: u64) -> Self {
        Self {
            window: std::time::Duration::from_secs(window_secs.max(1)),
            max_runs,
            max_fuel,
            entries: HashMap::new(),
            path: None,
            anchor_path: None,
            identity: None,
            deny: None,
        }
    }

    /// `NOSD_SKILL_QUOTA_{RUNS,FUEL,WINDOW_SECS}`. `0` é política válida
    /// (nega tudo); só valores não-numéricos caem no default.
    pub fn from_env() -> Self {
        Self::from_env_with_limits(Self::env_limits())
    }

    /// Quota persistida em `path` (criada do zero se o ficheiro não existir).
    /// Com `identity`, a janela é assinada e a âncora fica em
    /// `skill_quota.anchor` ao lado.
    pub fn from_env_at(path: std::path::PathBuf, identity: Option<nos_mesh::Identity>) -> Self {
        let mut q = Self::from_env_with_limits(Self::env_limits());
        q.anchor_path = Some(path.with_extension("anchor"));
        q.path = Some(path);
        q.identity = identity;
        q.load();
        q
    }

    /// A janela está assinada com a chave do nó?
    pub fn signed(&self) -> bool {
        self.identity.is_some()
    }

    fn env_limits() -> (u64, u32, u64) {
        fn env_u64(key: &str, default: u64) -> u64 {
            std::env::var(key)
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(default)
        }
        (
            env_u64("NOSD_SKILL_QUOTA_WINDOW_SECS", 60),
            env_u64("NOSD_SKILL_QUOTA_RUNS", 240).min(u32::MAX as u64) as u32,
            env_u64("NOSD_SKILL_QUOTA_FUEL", 5_000_000),
        )
    }

    fn from_env_with_limits(limits: (u64, u32, u64)) -> Self {
        let (window, runs, fuel) = limits;
        Self::new(window, runs, fuel)
    }

    /// Motivo da recusa global, se houver (para o operador ver no arranque).
    pub fn deny_reason(&self) -> Option<&str> {
        self.deny.as_deref()
    }

    /// Verifica a assinatura do ficheiro (quando há identidade) e devolve os
    /// peers. Sem identidade, a assinatura não é exigida (mas também não é
    /// falsificável como prova: `signed()` diz qual dos modos está activo).
    fn verified_peers(
        &self,
        f: &QuotaFile,
        what: &str,
    ) -> Result<HashMap<String, QuotaWindow>, String> {
        let Some(id) = &self.identity else {
            return Ok(f.peers.clone());
        };
        let Some(sig_hex) = f.sig.as_deref() else {
            return Err(format!(
                "{what} sem assinatura do no (identidade presente); recupere-o ou apague-o"
            ));
        };
        if f.node != Some(id.node_id()) {
            let signer = f.node.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
            return Err(format!(
                "{what} assinada pelo no {signer}, nao por {}; nao se aceita orcamento de outra chave",
                id.node_id()
            ));
        }
        let Some(sig) = unhex(sig_hex) else {
            return Err(format!("{what} com assinatura malformada"));
        };
        if !nos_mesh::ident::verify(
            &id.public_key(),
            &quota_canonical(f.window_secs, &f.peers),
            &sig,
        ) {
            return Err(format!(
                "{what} com assinatura invalida (ficheiro editado?); nao repoe orcamento"
            ));
        }
        Ok(f.peers.clone())
    }

    /// Impede recuar a janela: um ficheiro antigo (mas assinado) não pode
    /// devolver runs/fuel já gastos, nem fazer desaparecer um peer activo.
    fn enforce_anchor(&self, anchor: &QuotaFile, what: &str) -> Result<(), String> {
        let window_ms = self.window.as_millis() as u64;
        let now = now_ms();
        for (peer, a) in &anchor.peers {
            if now.saturating_sub(a.since_ms) >= window_ms {
                continue; // janela do anchor já expirou: nada a comparar
            }
            let Some(w) = self.entries.get(peer) else {
                return Err(format!(
                    "{what}: janela do peer {peer} desapareceu (rollback da quota)"
                ));
            };
            if w.since_ms < a.since_ms {
                return Err(format!(
                    "{what}: janela do peer {peer} recuou no tempo (rollback)"
                ));
            }
            if w.since_ms == a.since_ms && (w.runs < a.runs || w.fuel < a.fuel) {
                return Err(format!(
                    "{what}: quota do peer {peer} reduzida ({} runs/{} fuel < {} runs/{} fuel)",
                    w.runs, w.fuel, a.runs, a.fuel
                ));
            }
        }
        Ok(())
    }

    /// Lê e valida um ficheiro (janela ou âncora).
    fn read_checked(&mut self, path: &std::path::Path, what: &str) -> Option<QuotaFile> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.set_deny(format!(
                    "{what} ilegivel em {}: {e}; remova o ficheiro para recomecar",
                    path.display()
                ));
                return None;
            }
        };
        let f = match serde_json::from_str::<QuotaFile>(&text) {
            Ok(f) => f,
            Err(e) => {
                self.set_deny(format!(
                    "{what} corrompida em {}: {e}; remova o ficheiro para recomecar",
                    path.display()
                ));
                return None;
            }
        };
        if f.version != QUOTA_FILE_VERSION {
            self.set_deny(format!(
                "{what} em {} com versao {} desconhecida; remova o ficheiro",
                path.display(),
                f.version
            ));
            return None;
        }
        Some(f)
    }

    fn set_deny(&mut self, reason: String) {
        if self.deny.is_none() {
            self.deny = Some(reason);
        }
    }

    /// Carrega a janela do disco. Ausente = instalação nova **só** se a âncora
    /// também não existir; apagar a janela com a âncora presente é recusa
    /// (não é um orçamento novo em silêncio).
    fn load(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let anchor = match self.anchor_path.clone() {
            Some(a) if a.exists() => match self.read_checked(&a, "ancora da quota de skills") {
                Some(f) => Some(f),
                None => return, // recusa já registada
            },
            _ => None,
        };

        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if anchor.is_some() {
                    self.set_deny(format!(
                        "quota de skills apagada em {} mas a ancora existe; apague tambem a ancora para recomecar",
                        path.display()
                    ));
                }
                return;
            }
            Err(e) => {
                self.set_deny(format!(
                    "quota de skills ilegivel em {}: {e}; remova o ficheiro para recomecar",
                    path.display()
                ));
                return;
            }
        };
        let file = match serde_json::from_str::<QuotaFile>(&text) {
            Ok(f) => f,
            Err(e) => {
                self.set_deny(format!(
                    "quota de skills corrompida em {}: {e}; remova o ficheiro para recomecar",
                    path.display()
                ));
                return;
            }
        };
        if file.version != QUOTA_FILE_VERSION {
            self.set_deny(format!(
                "quota de skills em {} com versao {} desconhecida; remova o ficheiro",
                path.display(),
                file.version
            ));
            return;
        }
        let peers = match self.verified_peers(&file, "quota de skills") {
            Ok(p) => p,
            Err(e) => {
                self.set_deny(e);
                return;
            }
        };
        if let Some(a) = &anchor {
            if let Err(e) = self.verified_peers(a, "ancora da quota de skills") {
                self.set_deny(e);
                return;
            }
        }
        self.entries = peers;
        self.prune();
        if let Some(a) = &anchor {
            if let Err(e) = self.enforce_anchor(a, "quota de skills") {
                self.entries.clear();
                self.set_deny(e);
            }
        }
    }

    /// Descarta janelas já expiradas (não gastam disco nem ficheiro).
    fn prune(&mut self) {
        let window_ms = self.window.as_millis() as u64;
        let now = now_ms();
        self.entries
            .retain(|_, w| now.saturating_sub(w.since_ms) < window_ms);
    }

    /// Grava a janela **assinada** e a âncora (tmp + fsync + rename + fsync do
    /// diretório, 0600). A ordem é janela → âncora: um crash no meio deixa a
    /// âncora mais atrasada, o que a verificação monotónica aceita; o
    /// contrário (âncora à frente da janela) faria o arranque recusar.
    fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        self.prune();
        let mut file = QuotaFile {
            version: QUOTA_FILE_VERSION,
            window_secs: self.window.as_secs(),
            peers: self.entries.clone(),
            node: None,
            sig: None,
        };
        if let Some(id) = &self.identity {
            let msg = quota_canonical(file.window_secs, &file.peers);
            file.node = Some(id.node_id());
            file.sig = Some(hex_of(&id.sign(&msg)));
        }
        let body = serde_json::to_string(&file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_atomic(&path, &body)?;
        if let Some(anchor) = self.anchor_path.clone() {
            write_atomic(&anchor, &body)?;
        }
        Ok(())
    }

    /// Grava depois de uma mutação. Falha em gravar = recusa as execuções
    /// seguintes: sem persistência, o restart devolveria o orçamento inteiro.
    fn persist(&mut self) {
        if self.path.is_none() || self.deny.is_some() {
            return;
        }
        if let Err(e) = self.save() {
            let path = self
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            self.deny = Some(format!("quota de skills nao persistida em {path}: {e}"));
        }
    }

    /// Verifica antes de executar, sem gastar a janela. A janela expira e
    /// recomeça do zero.
    pub fn check(&mut self, peer: &str) -> Result<(), String> {
        if let Some(reason) = self.deny.clone() {
            return Err(reason);
        }
        let max_runs = self.max_runs;
        let max_fuel = self.max_fuel;
        let secs = self.window.as_secs();
        let w = self.window_for(peer);
        if w.runs >= max_runs {
            return Err(format!(
                "quota de execucao excedida (peer {peer}: {max_runs} runs/{secs}s)"
            ));
        }
        if w.fuel >= max_fuel {
            return Err(format!(
                "quota de fuel excedida (peer {peer}: {max_fuel} fuel/{secs}s)"
            ));
        }
        Ok(())
    }

    /// Reserva uma execução (valida e JÁ conta a run) e persiste-a. Sem a
    /// reserva atómica, dois pedidos concorrentes passavam ambos no `check`
    /// antes de qualquer cobrança e estouravam o limite de runs; sem a gravação
    /// um restart devolvia o orçamento.
    pub fn reserve(&mut self, peer: &str) -> Result<(), String> {
        self.check(peer)?;
        let w = self.window_for(peer);
        w.runs = w.runs.saturating_add(1);
        self.persist();
        match self.deny.clone() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    /// Cobra o fuel medido depois da execução (desconhecido = 0) e persiste. O
    /// fuel não pode ser reservado antes: o excesso possível fica limitado a
    /// uma run. Uma falha de gravação não nega o trabalho já feito — nega o
    /// próximo (`persist` deixa o motivo em `deny`).
    pub fn charge_fuel(&mut self, peer: &str, fuel: u64) {
        if self.deny.is_some() {
            return;
        }
        let w = self.window_for(peer);
        w.fuel = w.fuel.saturating_add(fuel);
        self.persist();
    }

    /// Janela do peer, criada/reiniciada se expirou.
    fn window_for(&mut self, peer: &str) -> &mut QuotaWindow {
        let now = now_ms();
        let window_ms = self.window.as_millis() as u64;
        let w = self.entries.entry(peer.to_string()).or_insert(QuotaWindow {
            since_ms: now,
            runs: 0,
            fuel: 0,
        });
        if now.saturating_sub(w.since_ms) >= window_ms {
            *w = QuotaWindow {
                since_ms: now,
                runs: 0,
                fuel: 0,
            };
        }
        w
    }
}

/// Telemetria local de fuel por skill (`skill/<nome>/fuel`). É uma observação
/// deste nó (replicada como estado, não autoridade global).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct FuelStat {
    pub runs: u64,
    pub total: u64,
    pub max: u64,
    pub limit: u64,
}

fn host_granted_caps() -> Vec<String> {
    std::env::var(HOST_GRANT_ENV)
        .unwrap_or_else(|_| "log".to_string())
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Interseção explícita. Uma cap declarada mas não autorizada pelo host fica
/// de fora (e é reportada em `skill.run` como `caps_declared` sem grant).
pub fn grants_for(declared: &[String]) -> SkillGrants {
    let host = host_granted_caps();
    let mut g = SkillGrants {
        bits: CapBits::NONE,
        log: false,
        names: Vec::new(),
    };
    for cap in declared {
        let c = cap.trim().to_ascii_lowercase();
        if !host.iter().any(|h| h == &c) {
            continue;
        }
        match c.as_str() {
            "aios_ai.infer" => {
                g.bits = g.bits.union(CapBits::INFER);
                g.names.push(c);
            }
            "log" => {
                g.log = true;
                g.names.push(c);
            }
            _ => {}
        }
    }
    g
}

/// Pedido de instalação. Um só tipo para o RPC (`origin=local`) e para o
/// hook do `skill.push` (`origin=peer`): a verificação não é duplicada.
pub struct InstallRequest<'a> {
    pub name: &'a str,
    pub focus: &'a str,
    pub version: &'a str,
    pub caps: &'a [String],
    pub publisher: Option<&'a str>,
    pub sig: Option<&'a str>,
    pub expect_hash: Option<&'a str>,
    pub allow_downgrade: bool,
    pub unsigned_provenance: &'a str,
    /// `frugal` (default) ou `app`; o host só aplica `app` com opt-in.
    pub profile: &'a str,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstalledSkill {
    pub name: String,
    pub focus: String,
    pub version: String,
    pub content_id: String,
    pub bytes: usize,
    pub abi: u32,
    pub provenance: String,
    pub caps: Vec<String>,
    pub profile: String,
    pub profile_requested: String,
    pub path: String,
}

/// Instala/activa um módulo. Ordem fail-closed: nome → tamanho/hash →
/// publisher → caps → downgrade → inspeção/execução → gravação → CRDT.
pub fn install(
    state: &AppState,
    req: &InstallRequest<'_>,
    bytes: &[u8],
) -> Result<InstalledSkill, String> {
    if req.name.is_empty() || req.name.contains('/') || req.name.len() > 64 {
        return Err("nome invalido".into());
    }
    if !matches!(req.profile, "frugal" | "app") {
        return Err("profile invalido (frugal|app)".into());
    }
    let (rt, profile) = runtime_for(req.profile);
    let policy = rt.policy();
    if bytes.len() > policy.max_module_bytes {
        return Err(format!(
            "modulo acima de {} bytes (politica {})",
            bytes.len(),
            policy.max_module_bytes
        ));
    }
    let cid = content_id(bytes);
    if let Some(expect) = req.expect_hash {
        if !expect.eq_ignore_ascii_case(&cid) {
            return Err(format!("hash nao bate (esperado {expect}, recebido {cid})"));
        }
    }
    let focus = if req.focus.is_empty() {
        req.name
    } else {
        req.focus
    };
    let version = if req.version.is_empty() {
        "0.1.0"
    } else {
        req.version
    };

    // S2: autoria ANTES de qualquer escrita (fail-closed).
    let signed = verify_publisher(
        req.name,
        focus,
        version,
        &cid,
        req.publisher,
        req.sig,
        &revoked_set(state),
    )?;
    let provenance = if signed == "signed" {
        "signed"
    } else {
        req.unsigned_provenance
    };
    // S2b: caps declaradas (allowlist fechada).
    verify_caps(req.caps).map_err(|e| format!("caps: {e}"))?;
    // S2b: versão monotónica por focus; downgrade exige grant server-authored.
    if let Some(maxv) = max_version_for_focus(state, focus) {
        if ver_key(version) < ver_key(&maxv) && !req.allow_downgrade {
            return Err(format!(
                "downgrade recusado: {version} < {maxv} (focus {focus})"
            ));
        }
    }

    // Contrato estático: imports fora da allowlist morrem com nome.
    let inspection = rt
        .inspect(bytes)
        .map_err(|e| format!("modulo invalido: {e}"))?;
    if let Some(denied) = inspection.denied_imports.first() {
        return Err(format!("import recusado: {denied}"));
    }
    if !inspection.has_run {
        return Err("modulo sem export run(i32) -> i32".into());
    }
    // Executa uma vez com zero caps. Fuel não é erro de módulo: um loop pode
    // ser intencional. Qualquer outro trap/erro = módulo inválido.
    match rt.execute(bytes, Grants::NONE, 1, 0) {
        Ok(_) | Err(nos_wasm::WasmError::Fuel) => {}
        Err(e) => return Err(format!("modulo invalido: {e}")),
    }

    let dir = skills_dir(state);
    std::fs::create_dir_all(&dir).map_err(|e| format!("skills dir: {e}"))?;
    let path = dir.join(format!("{cid}.wasm"));
    std::fs::write(&path, bytes).map_err(|e| format!("write: {e}"))?;
    state.mesh.crdt_put(
        state.mesh.local_id(),
        &format!("skill/{}/active", req.name),
        &cid,
        0,
    );
    put_meta(
        state,
        &SkillMeta {
            name: req.name,
            focus,
            version,
            hash: &cid,
            bytes: bytes.len(),
            provenance,
            caps: req.caps,
            kind: "wasm",
            abi: Some(inspection.abi),
            profile: req.profile,
            publisher: if provenance == "signed" {
                req.publisher
            } else {
                None
            },
            sig: if provenance == "signed" {
                req.sig
            } else {
                None
            },
        },
    );
    Ok(InstalledSkill {
        name: req.name.to_string(),
        focus: focus.to_string(),
        version: version.to_string(),
        content_id: cid,
        bytes: bytes.len(),
        abi: inspection.abi,
        provenance: provenance.to_string(),
        caps: req.caps.to_vec(),
        profile: profile.to_string(),
        profile_requested: req.profile.to_string(),
        path: path.display().to_string(),
    })
}

pub(crate) fn dispatch(state: &AppState, req: Request, peer: &str) -> Option<Response> {
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
            let focus_e = if focus.is_empty() { name } else { focus };
            let version_e = if version.is_empty() { "0.1.0" } else { version };
            let profile = req
                .params
                .get("profile")
                .and_then(|v| v.as_str())
                .unwrap_or("frugal");
            if !matches!(profile, "frugal" | "app") {
                return Some(err(req.id, "skill.install: profile invalido (frugal|app)"));
            }
            // S2b: o nonce HITL só é consumido quando há mesmo downgrade.
            let allow_downgrade = match max_version_for_focus(state, focus_e) {
                Some(maxv) if ver_key(version_e) < ver_key(&maxv) => {
                    if !super::hitl_grant_ok(state, &req, "skill.downgrade") {
                        return Some(err(
                            req.id,
                            format!("downgrade recusado: {version_e} < {maxv} (focus {focus_e})"),
                        ));
                    }
                    true
                }
                _ => false,
            };
            let spec = InstallRequest {
                name,
                focus: focus_e,
                version: version_e,
                caps: &caps,
                publisher: req.params.get("publisher").and_then(|v| v.as_str()),
                sig: req.params.get("sig").and_then(|v| v.as_str()),
                expect_hash: None,
                allow_downgrade,
                unsigned_provenance: "local",
                profile,
            };
            match install(state, &spec, &bytes) {
                Ok(out) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "name": out.name,
                        "hash": out.content_id,
                        "bytes": out.bytes,
                        "focus": out.focus,
                        "version": out.version,
                        "provenance": out.provenance,
                        "caps": out.caps,
                        "abi": out.abi,
                        "profile": out.profile,
                        "profile_requested": out.profile_requested,
                        "path": out.path,
                    }),
                },
                Err(e) => err(req.id, format!("skill.install: {e}")),
            }
        }
        "skill.list" => {
            let store = state.mesh.crdt_snapshot();
            let mut items: Vec<serde_json::Value> = store
                .entries
                .iter()
                .filter_map(|(k, e)| {
                    let name = k.strip_prefix("skill/")?.strip_suffix("/active")?;
                    let meta = store
                        .entries
                        .get(&format!("skill/{name}/meta"))
                        .and_then(|m| serde_json::from_str::<serde_json::Value>(&m.value).ok());
                    Some(json!({
                        "name": name,
                        "hash": e.value,
                        "version": meta.as_ref().and_then(|m| m["version"].as_str()),
                        "focus": meta.as_ref().and_then(|m| m["focus"].as_str()),
                        "caps": meta.as_ref().and_then(|m| m.get("caps").cloned()),
                        "kind": meta.as_ref().and_then(|m| m["kind"].as_str()),
                        "provenance": meta.as_ref().and_then(|m| m["provenance"].as_str()),
                        "abi": meta.as_ref().and_then(|m| m["abi"].as_u64()),
                        "profile": meta.as_ref().and_then(|m| m["profile"].as_str()),
                    }))
                })
                .collect();
            items.sort_by(|a, b| {
                a["name"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["name"].as_str().unwrap_or(""))
            });
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
            let meta = state
                .mesh
                .crdt_get(&format!("skill/{name}/meta"))
                .and_then(|e| serde_json::from_str::<serde_json::Value>(&e.value).ok());
            // Uma skill declarativa não tem `skill/<n>/active`; se houver
            // active (wasm) ele manda, mesmo que um import declarativo
            // posterior tenha reescrito o manifesto com outro `kind`.
            let declared: Vec<String> = meta
                .as_ref()
                .and_then(|m| m["caps"].as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let grants = grants_for(&declared);
            let requested = meta
                .as_ref()
                .and_then(|m| m["profile"].as_str())
                .unwrap_or("frugal");
            let (rt, profile) = runtime_for(requested);
            // Quota por peer: reserva a run ANTES de ler/executar (atómico sob
            // o mutex); o fuel é cobrado depois com o valor medido.
            if let Err(e) = state.skill_quota.lock().reserve(peer) {
                return Some(err(req.id, e));
            }
            let path = skills_dir(state).join(format!("{}.wasm", entry.value));
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return Some(err(req.id, format!("modulo {}: {e}", entry.value))),
            };
            // ABI v2: fuel medido, logs limitados, result buffer bounds-checado.
            let outcome = rt.execute(&bytes, Grants::new(grants.bits, grants.log), 1, arg);
            // Cobra a quota com o fuel observado; fuel trap = limite do perfil.
            let charged = match &outcome {
                Ok(r) => r.fuel_used,
                Err(nos_wasm::WasmError::Fuel) => rt.policy().fuel,
                Err(_) => 0,
            };
            state.skill_quota.lock().charge_fuel(peer, charged);
            // S2c: historico local + telemetria de eficiência para o score.
            bump_stats(state, name, outcome.is_ok());
            match &outcome {
                Ok(r) => bump_fuel(state, name, r.fuel_used, r.fuel_limit),
                Err(nos_wasm::WasmError::Fuel) => {
                    bump_fuel(state, name, rt.policy().fuel, rt.policy().fuel)
                }
                Err(_) => {}
            }
            match outcome {
                Ok(r) => {
                    let mut result = json!({
                        "name": name,
                        "arg": arg,
                        "value": r.value,
                        "hash": entry.value,
                        "abi": r.abi,
                        "profile": profile,
                        "profile_requested": requested,
                        "fuel_used": r.fuel_used,
                        "fuel_limit": r.fuel_limit,
                        "caps_declared": declared,
                        "caps_granted": grants.names,
                        "output": r.output_text(),
                        "output_truncated": r.output_truncated,
                    });
                    if let Some(text) = r.result_text() {
                        result["result"] = json!(text);
                    } else if let Some(bytes) = r.result.as_ref() {
                        use base64::Engine as _;
                        result["result_b64"] =
                            json!(base64::engine::general_purpose::STANDARD.encode(bytes));
                    }
                    result["result_truncated"] = json!(r.result_truncated);
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result,
                    }
                }
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({
                        "name": name,
                        "hash": entry.value,
                        "caps_declared": declared,
                        "caps_granted": grants.names,
                    }),
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
            let mut stats: HashMap<String, (u64, u64)> = HashMap::new();
            let mut fuel: HashMap<String, FuelStat> = HashMap::new();
            for (k, e) in &snapshot.entries {
                if let Some(n) = k
                    .strip_prefix("skill/")
                    .and_then(|r| r.strip_suffix("/stats"))
                {
                    if let Ok(v) = serde_json::from_str::<(u64, u64)>(&e.value) {
                        stats.insert(n.to_string(), v);
                    }
                } else if let Some(n) = k
                    .strip_prefix("skill/")
                    .and_then(|r| r.strip_suffix("/fuel"))
                {
                    if let Ok(v) = serde_json::from_str::<FuelStat>(&e.value) {
                        fuel.insert(n.to_string(), v);
                    }
                }
            }
            let metas = candidate_metas(state, focus);
            if metas.is_empty() {
                return Some(err(req.id, format!("sem candidatas para focus '{focus}'")));
            }
            let mut best: Option<(u32, serde_json::Value, serde_json::Value)> = None;
            for m in &metas {
                let (s, why) = score_meta(m, &metas, &stats, &fuel);
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
        // V6 (plano vampiro): importa `SKILL.md` declarativo (agentskills.io).
        // Não activa nada: grava manifesto S1 (`skill/<n>/meta`) com caps numa
        // allowlist e `kind=declarative`. Sem wasm ⇒ não executável.
        "skill.import" => {
            let raw = req
                .params
                .get("skill_md")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if raw.is_empty() {
                return Some(err(req.id, "skill.import: skill_md vazio"));
            }
            if raw.len() > 256 * 1024 {
                return Some(err(req.id, "skill.import: skill_md > 256 KiB"));
            }
            let doc = match super::skill_md::parse_skill_md(raw) {
                Ok(d) => d,
                Err(e) => return Some(err(req.id, e)),
            };
            let focus = req
                .params
                .get("focus")
                .and_then(|v| v.as_str())
                .unwrap_or(doc.focus.as_str())
                .to_string();
            let hash = nos_mesh::content_hash(raw.as_bytes());
            put_meta(
                state,
                &SkillMeta {
                    name: &doc.name,
                    focus: &focus,
                    version: &doc.version,
                    hash: &hash,
                    bytes: raw.len(),
                    provenance: "declarative",
                    caps: &doc.caps,
                    kind: "declarative",
                    abi: None,
                    profile: "",
                    publisher: None,
                    sig: None,
                },
            );
            state.mesh.crdt_put(
                state.mesh.local_id(),
                &format!("skill/{}/kind", doc.name),
                "declarative",
                0,
            );
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "name": doc.name,
                    "focus": focus,
                    "version": doc.version,
                    "caps": doc.caps,
                    "hash": hash,
                    "bytes": raw.len(),
                    "kind": "declarative",
                    "active": false,
                    "honesty": "sem wasm: não executável; caps declaradas no manifesto S1/S2",
                }),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod quota_tests {
    use super::*;

    /// Caminho único em `temp_dir` (sem dependência extra no crate).
    fn tmp_quota(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "nosd-quota-{}-{nanos}-{name}.json",
            std::process::id()
        ))
    }

    fn ident(seed: u8) -> nos_mesh::Identity {
        nos_mesh::Identity::from_seed([seed; 32], std::collections::HashSet::new(), true)
    }

    /// Quota persistida em `path` + âncora, com os limites de teste
    /// (60 s / 2 runs / 1000 fuel), já carregada do disco.
    fn quota_at(path: &std::path::Path, identity: Option<nos_mesh::Identity>) -> SkillQuota {
        let mut q = SkillQuota::new(60, 2, 1_000);
        q.anchor_path = Some(path.with_extension("anchor"));
        q.path = Some(path.to_path_buf());
        q.identity = identity;
        q.load();
        q
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("anchor"));
    }

    #[test]
    fn quota_window_survives_a_restart() {
        let path = tmp_quota("restart");
        let mut q = quota_at(&path, None);
        assert!(
            !q.signed(),
            "sem identidade, a janela é declaradamente não assinada"
        );
        assert!(q.reserve("unix:1000").is_ok());
        q.charge_fuel("unix:1000", 10);
        assert!(q.reserve("unix:1000").is_ok());
        q.charge_fuel("unix:1000", 20);
        assert!(q.reserve("unix:1000").is_err());
        drop(q);

        // Novo processo (simulado): mesma janela, mesmo orçamento gasto.
        let mut r = quota_at(&path, None);
        assert_eq!(r.deny_reason(), None, "ficheiro válido não nega");
        let e = r.reserve("unix:1000").unwrap_err();
        assert!(e.contains("quota"), "restart não devolve orçamento: {e}");
        // O fuel também sobreviveu: outro peer não herda a janela alheia.
        assert!(r.reserve("unix:2000").is_ok());
        // E o fuel persistido continua a contar para o peer original.
        assert_eq!(r.entries["unix:1000"].fuel, 30);
        assert_eq!(r.entries["unix:1000"].runs, 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "janela da quota é 0600");
        }
        cleanup(&path);
    }

    #[test]
    fn signed_window_survives_a_restart_and_is_ed25519() {
        let path = tmp_quota("signed-restart");
        let key = ident(7);
        let mut q = quota_at(&path, Some(key.clone()));
        assert!(q.signed());
        assert!(q.reserve("tcp:token:1").is_ok());
        q.charge_fuel("tcp:token:1", 5);
        drop(q);

        let raw: QuotaFile =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw.version, 2);
        assert_eq!(raw.node, Some(key.node_id()));
        assert_eq!(raw.sig.as_deref().map(str::len), Some(128), "Ed25519 hex");

        let r = quota_at(&path, Some(key));
        assert_eq!(r.deny_reason(), None);
        assert_eq!(r.entries["tcp:token:1"].runs, 1);
        assert_eq!(r.entries["tcp:token:1"].fuel, 5);
        cleanup(&path);
    }

    #[test]
    fn editing_the_signed_window_is_refused() {
        let path = tmp_quota("edited");
        let key = ident(7);
        let mut q = quota_at(&path, Some(key.clone()));
        assert!(q.reserve("p").is_ok());
        drop(q);

        // Edita a janela para devolver orçamento (mantém a assinatura antiga).
        let mut raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        raw["peers"]["p"]["runs"] = json!(0);
        raw["peers"]["p"]["since_ms"] = json!(now_ms());
        std::fs::write(&path, serde_json::to_string(&raw).unwrap()).unwrap();

        let mut r = quota_at(&path, Some(key));
        let e = r.reserve("p").unwrap_err();
        assert!(e.contains("assinatura"), "edição não repõe orçamento: {e}");
        cleanup(&path);
    }

    #[test]
    fn signed_window_without_signature_is_refused() {
        let path = tmp_quota("unsigned");
        let key = ident(7);
        let mut q = quota_at(&path, Some(key.clone()));
        assert!(q.reserve("p").is_ok());
        drop(q);

        let mut raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        raw.as_object_mut().unwrap().remove("sig");
        std::fs::write(&path, serde_json::to_string(&raw).unwrap()).unwrap();

        let mut r = quota_at(&path, Some(key));
        let e = r.reserve("p").unwrap_err();
        assert!(e.contains("sem assinatura"), "{e}");
        cleanup(&path);
    }

    #[test]
    fn window_signed_by_another_node_is_refused() {
        let path = tmp_quota("foreign");
        let mut q = quota_at(&path, Some(ident(8)));
        assert!(q.reserve("p").is_ok());
        drop(q);

        let mut r = quota_at(&path, Some(ident(7)));
        let e = r.reserve("p").unwrap_err();
        assert!(e.contains("outra chave"), "{e}");
        cleanup(&path);
    }

    #[test]
    fn deleting_the_window_with_the_anchor_present_is_refused() {
        let path = tmp_quota("deleted");
        let key = ident(7);
        let mut q = quota_at(&path, Some(key.clone()));
        assert!(q.reserve("p").is_ok());
        drop(q);

        std::fs::remove_file(&path).unwrap();
        let mut r = quota_at(&path, Some(key));
        let e = r.reserve("p").unwrap_err();
        assert!(
            e.contains("apagada"),
            "apagar a janela não dá orçamento novo: {e}"
        );
        assert!(
            path.with_extension("anchor").exists(),
            "a âncora continua lá"
        );
        cleanup(&path);
    }

    #[test]
    fn rollback_to_an_older_signed_window_is_refused() {
        let path = tmp_quota("rollback");
        let key = ident(7);
        let mut q = quota_at(&path, Some(key.clone()));
        assert!(q.reserve("p").is_ok());
        let older = std::fs::read(&path).unwrap();
        assert!(q.reserve("p").is_ok()); // segunda run → âncora sobe
        drop(q);

        // Repõe o ficheiro assinado antigo; a âncora (mais recente) recusa.
        std::fs::write(&path, older).unwrap();
        let mut r = quota_at(&path, Some(key));
        let e = r.reserve("p").unwrap_err();
        assert!(e.contains("reduzida") || e.contains("rollback"), "{e}");
        cleanup(&path);
    }

    #[test]
    fn corrupt_quota_file_denies_instead_of_granting_a_fresh_budget() {
        let path = tmp_quota("corrupt");
        std::fs::write(&path, "{not json").unwrap();
        let mut q = quota_at(&path, None);
        let e = q.reserve("p").unwrap_err();
        assert!(e.contains("corrompida"), "fail-closed: {e}");
        assert!(q.reserve("outro").is_err(), "a recusa é global");
        cleanup(&path);
    }

    #[test]
    fn expired_window_is_pruned_on_load() {
        let path = tmp_quota("expired");
        let old = QuotaFile {
            version: QUOTA_FILE_VERSION,
            window_secs: 60,
            peers: HashMap::from([(
                "p".to_string(),
                QuotaWindow {
                    since_ms: now_ms().saturating_sub(120_000),
                    runs: 9,
                    fuel: 999,
                },
            )]),
            node: None,
            sig: None,
        };
        std::fs::write(&path, serde_json::to_string(&old).unwrap()).unwrap();
        let mut q = quota_at(&path, None);
        assert!(q.entries.is_empty(), "janela antiga descartada no load");
        assert!(q.reserve("p").is_ok());
        cleanup(&path);
    }

    #[test]
    fn quota_limits_runs_and_fuel_per_peer() {
        let mut q = SkillQuota::new(60, 2, 100);
        assert!(q.reserve("unix:1").is_ok());
        q.charge_fuel("unix:1", 10);
        assert!(q.reserve("unix:1").is_ok());
        q.charge_fuel("unix:1", 20);
        let e = q.reserve("unix:1").unwrap_err();
        assert!(e.contains("quota"), "{e}");
        // Outro peer tem a sua própria janela, não herda a quota.
        assert!(q.reserve("unix:2").is_ok());
    }

    #[test]
    fn quota_fuel_trips_even_with_runs_left() {
        let mut q = SkillQuota::new(60, 100, 10);
        assert!(q.reserve("p").is_ok());
        q.charge_fuel("p", 10);
        let e = q.reserve("p").unwrap_err();
        assert!(e.contains("fuel"), "{e}");
    }

    #[test]
    fn quota_reserves_before_the_run_so_concurrent_calls_cannot_overshoot() {
        // max_runs=1: o 1º reserva, o 2º já é recusado mesmo sem fuel cobrado.
        let mut q = SkillQuota::new(60, 1, 1_000);
        assert!(q.reserve("p").is_ok());
        let e = q.reserve("p").unwrap_err();
        assert!(e.contains("quota"), "{e}");
        // A janela expira e o peer volta a poder executar.
        let mut q = SkillQuota::new(1, 1, 1_000);
        assert!(q.reserve("p").is_ok());
        std::thread::sleep(std::time::Duration::from_millis(1_050));
        assert!(q.reserve("p").is_ok(), "janela nova depois do TTL");
    }

    #[test]
    fn quota_zero_runs_denies_every_peer() {
        // `0` é política válida (nega tudo), não "sem limite".
        let mut q = SkillQuota::new(60, 0, 1_000);
        assert!(q.reserve("p").is_err());
        assert!(q.reserve("outro").is_err());
    }
}

#[cfg(test)]
mod score_tests {
    use super::*;

    #[test]
    fn fuel_efficiency_changes_the_score() {
        let meta = json!({
            "name": "a", "focus": "f", "version": "1.0.0",
            "caps": [], "bytes": 100, "provenance": "local"
        });
        let metas = vec![meta.clone()];
        let stats = HashMap::new();
        let mut fuel = HashMap::new();

        // Alvo = 10% de 1000 = 100; média 10 → eficiência máxima.
        fuel.insert(
            "a".to_string(),
            FuelStat {
                runs: 3,
                total: 30,
                max: 10,
                limit: 1000,
            },
        );
        let (cheap, why) = score_meta(&meta, &metas, &stats, &fuel);
        assert_eq!(why["fuel"], 10, "{why}");
        assert_eq!(why["fuel_avg"], 10, "{why}");

        // Média 1000 no mesmo limite → 1 ponto; total desce.
        fuel.insert(
            "a".to_string(),
            FuelStat {
                runs: 3,
                total: 3000,
                max: 1000,
                limit: 1000,
            },
        );
        let (dear, why2) = score_meta(&meta, &metas, &stats, &fuel);
        assert_eq!(why2["fuel"], 1, "{why2}");
        assert!(cheap > dear, "{cheap} vs {dear}");

        // Menos de 3 execuções: neutro, não premeia uma amostra só.
        fuel.insert(
            "a".to_string(),
            FuelStat {
                runs: 2,
                total: 20,
                max: 10,
                limit: 1000,
            },
        );
        let (_, why3) = score_meta(&meta, &metas, &stats, &fuel);
        assert_eq!(why3["fuel"], 5, "{why3}");
    }
}

#[cfg(test)]
mod v2_tests {
    use super::*;

    #[test]
    fn grants_are_intersection_of_declared_and_host_policy() {
        std::env::remove_var(HOST_GRANT_ENV);
        // Default: `log` pode; `infer` exige opt-in do operador.
        assert_eq!(
            grants_for(&[]),
            SkillGrants {
                bits: CapBits::NONE,
                log: false,
                names: vec![]
            }
        );
        let g = grants_for(&["log".into()]);
        assert!(g.log);
        assert_eq!(g.bits, CapBits::NONE);
        assert_eq!(g.names, vec!["log".to_string()]);
        let g = grants_for(&["aios_ai.infer".into()]);
        assert!(!g.log);
        assert_eq!(g.bits, CapBits::NONE, "sem opt-in nao ha INFER");
        assert!(g.names.is_empty());

        // Com opt-in explícito, a cap declarada passa.
        std::env::set_var(HOST_GRANT_ENV, "log,aios_ai.infer");
        let g = grants_for(&["aios_ai.infer".into()]);
        assert_eq!(g.bits, CapBits::INFER);
        assert!(g.names.contains(&"aios_ai.infer".to_string()));
        // Cap fora da allowlist do host continua de fora.
        let g = grants_for(&["fs.read".into()]);
        assert_eq!(g.bits, CapBits::NONE);
        assert!(g.names.is_empty());
        std::env::remove_var(HOST_GRANT_ENV);
    }

    #[test]
    fn push_payload_carries_declared_caps() {
        let meta = json!({
            "focus": "sum",
            "version": "3.0.0",
            "caps": ["log"],
            "kind": "wasm"
        });
        let p = push_payload(Some(&meta), "sum", "hash", "d2FzbQ==");
        assert_eq!(p["caps"], json!(["log"]));
        assert_eq!(p["kind"], "wasm");
    }
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
            verify_publisher(
                "sum",
                "sum",
                "1.0.0",
                &hash,
                Some(&pk_hex),
                Some(&sig_b64),
                &Default::default()
            ),
            Ok("signed")
        );
        // Manifesto adulterado (versao diferente) deixa de bater.
        let bad = verify_publisher(
            "sum",
            "sum",
            "2.0.0",
            &hash,
            Some(&pk_hex),
            Some(&sig_b64),
            &Default::default(),
        );
        assert!(bad.is_err(), "{bad:?}");
        // Sem assinatura = local (nao e' "signed").
        assert_eq!(
            verify_publisher(
                "sum",
                "sum",
                "1.0.0",
                &hash,
                None,
                None,
                &Default::default()
            ),
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

#[cfg(test)]
mod s2c_tests {
    use super::*;

    #[test]
    fn meta_records_publisher_and_signature_when_signed() {
        let meta = json!({ "name": "sum", "focus": "sum", "version": "1.0.0" });
        let m = meta_with_auth(meta.clone(), Some("aa".repeat(32).as_str()), Some("c2ln"));
        assert_eq!(m["publisher"], "aa".repeat(32));
        assert_eq!(m["sig"], "c2ln");
        // Sem autoria, o manifesto fica intacto.
        let m2 = meta_with_auth(meta, None, None);
        assert!(m2.get("publisher").is_none());
        assert!(m2.get("sig").is_none());
    }
}

#[cfg(test)]
mod s2c_push_tests {
    use super::*;

    #[test]
    fn push_payload_carries_authorship() {
        let meta = json!({ "focus": "sum", "version": "2.0.0", "publisher": "ab", "sig": "c2ln" });
        let p = push_payload(Some(&meta), "sum", "hash", "d2FzbQ==");
        assert_eq!(p["publisher"], "ab");
        assert_eq!(p["sig"], "c2ln");
        assert_eq!(p["focus"], "sum");
        assert_eq!(p["version"], "2.0.0");
        // Sem manifesto, o payload nao inventa autoria.
        let q = push_payload(None, "x", "hash", "d2FzbQ==");
        assert!(q.get("publisher").is_none());
        assert!(q.get("sig").is_none());
    }
}

#[cfg(test)]
mod s2c_revoke_tests {
    use super::*;
    use base64::Engine as _;
    use nos_mesh::ident::Identity;

    #[test]
    fn revoked_publisher_is_refused_even_with_valid_signature() {
        let id = Identity::from_seed([93u8; 32], Default::default(), true);
        let pk_hex: String = id.public_key().iter().map(|b| format!("{b:02x}")).collect();
        let hash = "b".repeat(64);
        let msg = publisher_msg("s", "s", "1.0.0", &hash);
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(id.sign(msg.as_bytes()));

        // Sem revogacao: aceita.
        assert_eq!(
            verify_publisher(
                "s",
                "s",
                "1.0.0",
                &hash,
                Some(&pk_hex),
                Some(&sig_b64),
                &Default::default()
            ),
            Ok("signed")
        );
        // Revogado: recusa apesar da assinatura ser valida.
        let revoked: std::collections::HashSet<String> = [pk_hex.clone()].into_iter().collect();
        let r = verify_publisher(
            "s",
            "s",
            "1.0.0",
            &hash,
            Some(&pk_hex),
            Some(&sig_b64),
            &revoked,
        );
        assert!(r.is_err(), "{r:?}");
        assert!(format!("{r:?}").contains("revogado"), "{r:?}");
    }
}
