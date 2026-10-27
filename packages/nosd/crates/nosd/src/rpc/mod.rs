//! RPC surface do nosd (0.5.4) — dispatch fatiado por domínio.
mod chat;
mod factory_store;
mod health;
mod http_lab;
mod model;
mod nix_wifi;
mod security;
mod ui_orb;
mod voice_transmute;

pub use http_lab::serve_orb_lab;

use crate::{actuators, nix_managed, transmute, wifi_nm};
use nos_core::{
    disposition, lab_auto_switch_from_env, note_violation, s1_backend_label, ActDisposition,
    ActDomain, CapBits, CapGate, CapRegistry, NosScore, PeerProfile, SafetyViolation, Sev,
    UiSource, WindowSpec,
};
use nos_cortex::CapacityProber;
use nos_factory::{ImportMeta, ImportProvenance, PackageStore, SandboxStatus, StoreError};
use nos_hermes::{
    deliberate_blob_fetch, iso_must_not_ship_blobs, remember_deliberation, BlobKind, BlobNeed,
    HermesEngine,
};
use nos_ipc::{decode_body, encode, take_frame, Request, Response};
use nos_mesh::{fleet_worst, HealthVerdict, MachineHealth, MeshEngine};
use parking_lot::{Mutex, RwLock};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn};

/// S-04: grants HITL server-authored. Nonce de uso único, TTL 5 min, kind-bound.
#[derive(Debug, Default)]
pub struct HitlGrantStore {
    grants: Mutex<HashMap<String, HitlGrant>>,
}

#[derive(Debug, Clone)]
struct HitlGrant {
    kind: String,
    expires_ms: u64,
}

impl HitlGrantStore {
    pub fn issue(&self, kind: &str) -> (String, u64) {
        let kind = if kind.is_empty() {
            "hitl.confirm"
        } else {
            kind
        };
        let now = now_ms();
        let expires_ms = now.saturating_add(5 * 60 * 1000);
        let id = uuid::Uuid::new_v4().simple().to_string();
        let mut grants = self.grants.lock();
        grants.retain(|_, g| g.expires_ms > now);
        if grants.len() >= 64 {
            if let Some(old) = grants.keys().next().cloned() {
                grants.remove(&old);
            }
        }
        grants.insert(
            id.clone(),
            HitlGrant {
                kind: kind.to_string(),
                expires_ms,
            },
        );
        (id, expires_ms)
    }

    pub fn consume(&self, id: &str, kind: &str) -> bool {
        if id.is_empty() {
            return false;
        }
        let now = now_ms();
        let mut grants = self.grants.lock();
        matches!(
            grants.remove(id),
            Some(g) if g.expires_ms > now && g.kind == kind
        )
    }

    pub fn len(&self) -> usize {
        self.grants.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// S-04: aceita `confirm_id` server-authored; `hitl_confirmed` só como
/// compatibilidade de LocalAdmin (root/token). Jarbas/LocalUser nunca usa bool.
fn hitl_grant_ok(state: &AppState, gate: &CapGate, req: &Request, kind: &str) -> bool {
    if let Some(id) = req.params.get("confirm_id").and_then(|v| v.as_str()) {
        return state.hitl.consume(id, kind);
    }
    gate.held.0 == CapBits::LEGIT_MASK
        && req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) == Some(true)
}

/// Estado do token TCP/HTTP. A rotação invalida tokens antigos.
#[derive(Debug, Clone)]
pub struct IpcAuth {
    pub token: String,
    pub generation: u64,
}

/// Identidade resolvida pelo transporte, nunca pelo payload.
#[derive(Debug, Clone, Copy)]
pub enum AuthContext {
    Unix { uid: Option<u32>, gid: Option<u32> },
    Tcp,
}

pub struct AppState {
    pub hermes: HermesEngine,
    pub mesh: MeshEngine,
    pub data_dir: PathBuf,
    pub ipc: RwLock<IpcAuth>,
    pub ipc_token_file: PathBuf,
    pub daemon_uid: u32,
    pub admin_gid: u32,
    /// S-06: ring Nix (root apply/rollback) desarmado por default.
    pub nix_ring_armed: bool,
    /// S-04: grants HITL single-use.
    pub hitl: HitlGrantStore,
    /// CapGate server-authored (I.1) - nunca confiar em req.caps/req.agent.
    pub caps: CapRegistry,
    pub package_store: Mutex<PackageStore>,
    pub shutting_down: AtomicBool,
    /// Contador do loop Continuous Hermes (0 = ainda não tickou / desligado).
    pub hermes_ticks: std::sync::atomic::AtomicU64,
}

impl AppState {
    pub fn ipc_generation(&self) -> u64 {
        self.ipc.read().generation
    }

    pub fn ipc_token(&self) -> String {
        self.ipc.read().token.clone()
    }

    /// Resolve perfil + geração a partir do transporte.
    /// Unix: `SO_PEERCRED`. TCP/HTTP: token de processo (comparação constante).
    pub fn authenticate(&self, auth: &AuthContext, token: Option<&str>) -> (PeerProfile, u64) {
        // Token-first em qualquer transporte: a CLI/operador prova posse do
        // ficheiro 0600 e recebe LocalAdmin; JARBAS não envia token e fica Jarbas.
        if let Some(t) = token {
            let ipc = self.ipc.read();
            return if nos_core::ct_eq(t.as_bytes(), ipc.token.as_bytes()) {
                (PeerProfile::LocalAdmin, ipc.generation)
            } else {
                (PeerProfile::Unknown, 0)
            };
        }
        match auth {
            AuthContext::Unix { uid, gid } => {
                let profile = match (uid, gid) {
                    (Some(uid), Some(gid)) => {
                        PeerProfile::from_unix_cred(*uid, *gid, self.daemon_uid, self.admin_gid)
                    }
                    (Some(uid), None) => {
                        PeerProfile::from_unix_cred(*uid, 0, self.daemon_uid, self.admin_gid)
                    }
                    _ => PeerProfile::Unknown,
                };
                (profile, self.ipc_generation())
            }
            AuthContext::Tcp => (PeerProfile::Unknown, 0),
        }
    }

    /// Rotaciona token + geração. Escreve o ficheiro antes de trocar em memória.
    pub fn rotate_ipc(&self) -> Result<(String, u64), std::io::Error> {
        let token = new_ipc_token();
        write_ipc_token(&self.ipc_token_file, &token)?;
        let mut ipc = self.ipc.write();
        ipc.token = token.clone();
        ipc.generation = ipc.generation.saturating_add(1);
        Ok((token, ipc.generation))
    }
}

pub fn new_ipc_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Escreve o token com modo 0600 em Unix (best-effort de permissões).
pub fn write_ipc_token(path: &Path, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(token.as_bytes())?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn opar_after_ok(
    state: &AppState,
    method: &str,
    ok: bool,
    observe: &str,
    plan: &str,
    act: &str,
    verify: &str,
) {
    if ok && nos_hermes::is_side_effect(method) {
        let _ = state
            .hermes
            .opar_side_effect(method, observe, plan, act, verify);
    }
}

fn note_capgate_deny(profile: PeerProfile, method: &str, detail: &str) {
    let _ = note_violation(SafetyViolation {
        invariant: "capgate".into(),
        agent: format!("{profile:?}"),
        skill: method.to_string(),
        detail: detail.to_string(),
    });
}

/// W4/W5 - score honesto: S1 label, llama-server posture, landlock, early boot JSON.
fn build_runtime_score(state: &AppState, phase: &str, msg: &str) -> NosScore {
    let fleet_verdict = state.mesh.fleet_verdict().map(|v| v.as_str().to_string());
    let llama = nos_cortex::llama_server_posture(state.hermes.hotswap.engine_name());
    let early_boot = {
        let p = state.data_dir.join("boot-score.json");
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    };
    NosScore {
        phase: phase.into(),
        sev: Sev::Ok,
        msg: msg.into(),
        hub_slots: state.hermes.hub.loaded_count(),
        memory_ok: state.hermes.memory.health(),
        mesh_peers: state.mesh.peer_count(),
        fleet_verdict,
        summary: String::new(),
        s1_backend: s1_backend_label().into(),
        llama_server: llama,
        landlock: format!("{:?}", crate::landlock::applied_status()),
        early_boot,
    }
    .with_summary()
}

pub async fn serve_tcp(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    loop {
        if state.shutting_down.load(Ordering::Relaxed) {
            break;
        }
        let (sock, _) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_stream(sock, st, AuthContext::Tcp).await {
                warn!(target: "nosd", "conn err: {e}");
            }
        });
    }
    Ok(())
}

#[cfg(unix)]
pub async fn serve_unix(path: String, state: Arc<AppState>) -> anyhow::Result<()> {
    // S-09: o socket é 0660 (dono nosd:grupo nosd). A autorização continua a
    // ser SO_PEERCRED + caps, mas fechar o modo evita que "outros" escrevam
    // no socket de um daemon que corre com privilégios largos.
    if let Some(dir) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let listener = tokio::net::UnixListener::bind(&path)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660))
            .map_err(|e| anyhow::anyhow!("S-09: chmod 0660 {path}: {e}"))?;
    }
    loop {
        if state.shutting_down.load(Ordering::Relaxed) {
            break;
        }
        let (sock, _) = listener.accept().await?;
        let st = state.clone();
        // SO_PEERCRED: a identidade é do kernel, não do JSON.
        let cred = sock.peer_cred().ok();
        let auth = AuthContext::Unix {
            uid: cred.as_ref().map(|c| c.uid()),
            gid: cred.as_ref().map(|c| c.gid()),
        };
        tokio::spawn(async move {
            if let Err(e) = handle_stream(sock, st, auth).await {
                warn!(target: "nosd", "conn err: {e}");
            }
        });
    }
    Ok(())
}

fn method_domain(method: &str) -> Option<ActDomain> {
    Some(match method {
        "chat" | "chat.submit" | "route.explain" => ActDomain::ChatRoute,
        "model.plan" | "model.suggest" => ActDomain::ModelPlanSuggest,
        "model.fetch" | "model.search" | "blob.deliberate" => ActDomain::ModelFetch,
        "firmware.fetch" => ActDomain::DeviceFirmware,
        "model.load" => ActDomain::HotSwapKnownHash,
        "wifi.scan" => ActDomain::WifiScan,
        "wifi.connect" => ActDomain::WifiConnect,
        "nix.propose" => ActDomain::NixPropose,
        "nix.apply" | "nix.confirm" | "nix.build_vm" => ActDomain::NixSwitch,
        "nix.rollback" => ActDomain::NixRollback,
        "agent.spawn" | "factory.promote" => ActDomain::AgentSpawnSigned,
        "factory.seek" | "store.search" => ActDomain::FactorySeekScan,
        "store.install" => ActDomain::FactorySeekInstall,
        "factory.synthesize" => ActDomain::FactorySynthesize,
        "systemctl.scoped" => ActDomain::SystemctlScoped,
        "audio.volume" => ActDomain::PipeWireVolume,
        "gpu.plan" => ActDomain::ModelPlanSuggest, // flake/ngl plan = Auto; power-limit = Deny domain
        "gpu.power_limit" => ActDomain::GpuPowerLimit,
        "device.recipe" => ActDomain::NixPropose,
        "self.heal.evaluate" => ActDomain::NixPropose,
        _ => return None,
    })
}

async fn handle_stream<S>(
    mut sock: S,
    state: Arc<AppState>,
    auth: AuthContext,
) -> anyhow::Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut acc = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = sock.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        acc.extend_from_slice(&buf[..n]);
        while let Some((body, consumed)) = take_frame(&acc)? {
            acc.drain(..consumed);
            let req: Request = decode_body(&body)?;
            let resp = dispatch(&state, &auth, req);
            sock.write_all(&encode(&resp)?).await?;
        }
    }
    Ok(())
}

/// Atualiza MCH local a partir de sinais runtime (honest partial - não substitui observe.snapshot).
pub fn sync_local_fleet_health(state: &AppState) {
    let ticks = state
        .hermes_ticks
        .load(std::sync::atomic::Ordering::Relaxed);
    let sys = if ticks > 0 {
        HealthVerdict::Go
    } else {
        HealthVerdict::Unknown
    };
    let hub = if state.hermes.hub.loaded_count() > 0 {
        HealthVerdict::Go
    } else {
        HealthVerdict::Unknown
    };
    state.mesh.set_local_fleet_health(MachineHealth {
        node_id: state.mesh.local_id().min(255) as u8,
        sys,
        audio: HealthVerdict::Unknown,
        hub,
        reason: format!(
            "hermes_ticks={} hub_loaded={}",
            ticks,
            state.hermes.hub.loaded_count()
        ),
    });
}

fn dispatch(state: &AppState, auth: &AuthContext, req: Request) -> Response {
    // I.1: identidade vem do transporte (peer creds / token); payload é ignorado.
    let (profile, auth_gen) = state.authenticate(auth, req.token.as_deref());
    if profile == PeerProfile::Unknown {
        note_capgate_deny(profile, &req.method, "unauthenticated peer");
        return Response {
            id: req.id,
            ok: false,
            error: Some("unauthenticated peer (peer creds / token required)".into()),
            result: json!({ "capgate": "server_authored", "profile": "Unknown" }),
        };
    }

    let current_gen = state.ipc_generation();
    let gate = state.caps.gate_for(profile, auth_gen);
    let need = req.required_cap();
    if need.0 != 0 {
        if let Err(e) = gate.allow(need, current_gen) {
            note_capgate_deny(profile, &req.method, e);
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("{e} [server-authored profile={profile:?}]")),
                result: json!({
                    "need": need.0,
                    "held": gate.held.0,
                    "profile": format!("{profile:?}"),
                }),
            };
        }
    }
    // NX.0: policy gate for destructive domains (before side-effects).
    if let Some(dom) = method_domain(&req.method) {
        let lab = lab_auto_switch_from_env();
        let disp = disposition(dom, lab);
        if disp == ActDisposition::Deny {
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("SITE_POLICY deny domain={dom:?}")),
                result: json!({ "disposition": "deny" }),
            };
        }
        // ModelFetch / DeviceFirmware: HITL via blob_deliberate (abre card JARBAS) - não short-circuit.
        let blob_domain = matches!(dom, ActDomain::ModelFetch | ActDomain::DeviceFirmware);
        if disp == ActDisposition::Escalate
            && !blob_domain
            && req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) != Some(true)
        {
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!(
                    "SITE_POLICY escalate domain={dom:?} - HITL required"
                )),
                result: json!({ "disposition": "escalate", "need_hitl": true }),
            };
        }
        if disp == ActDisposition::Confirm
            && matches!(
                dom,
                ActDomain::NixSwitch | ActDomain::WifiConnect | ActDomain::FactorySeekInstall
            )
            && req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) != Some(true)
            && !(lab && matches!(dom, ActDomain::NixSwitch))
        {
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("SITE_POLICY confirm domain={dom:?}")),
                result: json!({ "disposition": "confirm", "need_hitl": true }),
            };
        }
    }

    if let Some(r) = security::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = health::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = model::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = chat::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = nix_wifi::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = voice_transmute::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = ui_orb::dispatch(state, &gate, req.clone()) {
        return r;
    }
    if let Some(r) = factory_store::dispatch(state, &gate, req.clone()) {
        return r;
    }
    Response {
        id: req.id,
        ok: false,
        error: Some(format!("unknown method {}", req.method)),
        result: json!(null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitl_grant_is_single_use_and_kind_bound() {
        let store = HitlGrantStore::default();
        let (id, expires) = store.issue("self.heal");
        assert!(expires > 0);
        assert!(!store.consume(&id, "nix.apply"));

        let (id2, _) = store.issue("self.heal");
        assert!(store.consume(&id2, "self.heal"));
        assert!(!store.consume(&id2, "self.heal"));
        assert!(store.is_empty());
    }
}
