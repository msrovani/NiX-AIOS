//! M7 — Jobs remotos (NoProto sobre TCP) com frames assinados.
//!
//! M6.1: o TCP vai dentro de um canal **Noise XX** (sigilo + forward secrecy)
//! e a identidade Ed25519 é ligada ao handshake via `BIND` (ver `noise.rs`).
//!
//! Wire: `NJOB | ver u8 | len u32 LE | body | sig 64` (dentro do Noise).
//! O frame continua assinado (defesa em profundidade) e o recetor exige a
//! chave fixada no mesh.

use crate::ident::{short_id, Identity, PinError, Pinned};
use crate::noise::Secure;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

pub const MAGIC: &[u8] = b"NJOB";
pub const VER: u8 = 1;
pub const MAX_BODY: usize = 1 << 20; // 1 MiB por job (prompt + overhead)
pub const MAX_PROMPT: usize = 8 * 1024;
/// M9.1: limite real do módulo de skill no push (512 KiB).
pub const MAX_SKILL_BYTES: usize = 512 * 1024;
/// T0.1: o envelope de skill leva o módulo em base64 (~4/3 do tamanho), o que
/// não cabe no `MAX_PROMPT` de chat — o teto de 512 KiB era inalcançável
/// (qualquer módulo >~6 KiB morria em "prompt acima do limite" antes do
/// limite de módulo ser sequer consultado). Limite por kind, não global.
pub const MAX_SKILL_PAYLOAD: usize = 768 * 1024;
pub const DEFAULT_DEADLINE_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnvelope {
    pub job_id: [u8; 16],
    /// 1 = chat, 2 = crdt.
    pub kind: u8,
    pub prompt: String,
    pub deadline_ms: u64,
    pub budget_tokens: u32,
    /// Id derivado da chave do cliente.
    pub origin: u32,
    pub key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: [u8; 16],
    pub ok: bool,
    pub text: String,
    pub error: Option<String>,
    pub worker: u32,
    pub key: [u8; 32],
    pub elapsed_ms: u64,
}

pub fn new_job_id() -> [u8; 16] {
    let u = uuid::Uuid::new_v4();
    let mut out = [0u8; 16];
    out.copy_from_slice(u.as_bytes());
    out
}

/// Executor local (prompt, budget) -> texto.
pub type Executor = Arc<dyn Fn(&str, u32) -> Result<String, String> + Send + Sync>;

/// M8: hook de estado — recebe o payload e devolve o snapshot já merged.
pub type CrdtHook = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// M9.1: hook de skills — recebe {name,hash,wasm_b64} e activa se o hash bater.
pub type SkillHook = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

pub const KIND_CHAT: u8 = 1;
pub const KIND_CRDT: u8 = 2;
pub const KIND_SKILL: u8 = 3;

/// Limite do campo `prompt` por kind: chat/CRDT ficam no teto de chat;
/// skill tem teto próprio (base64 do módulo + manifesto).
pub fn max_prompt_for(kind: u8) -> usize {
    if kind == KIND_SKILL {
        MAX_SKILL_PAYLOAD
    } else {
        MAX_PROMPT
    }
}

fn frame(body: &[u8], id: &Identity) -> Vec<u8> {
    let mut v = MAGIC.to_vec();
    v.push(VER);
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(body);
    let sig = id.sign(&v);
    v.extend_from_slice(&sig);
    v
}

struct ParsedFrame {
    body: Vec<u8>,
    sig: [u8; 64],
    signed: Vec<u8>,
}

fn parse_frame(buf: &[u8]) -> Result<ParsedFrame, String> {
    if buf.len() < 4 + 1 + 4 + 64 {
        return Err("frame curto".into());
    }
    if &buf[0..4] != MAGIC {
        return Err("magic invalido".into());
    }
    if buf[4] != VER {
        return Err(format!("versao {}", buf[4]));
    }
    let len = u32::from_le_bytes(buf[5..9].try_into().unwrap()) as usize;
    if len > MAX_BODY {
        return Err("job demasiado grande".into());
    }
    if buf.len() != 9 + len + 64 {
        return Err("tamanho inconsistente".into());
    }
    let signed = buf[..9 + len].to_vec();
    let body = buf[9..9 + len].to_vec();
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&buf[9 + len..9 + len + 64]);
    Ok(ParsedFrame { body, sig, signed })
}

/// Servidor de jobs (Noise + frame assinado).
pub async fn serve(
    bind: &str,
    identity: Identity,
    pinned: Arc<Pinned>,
    exec: Executor,
    crdt: CrdtHook,
    skill: SkillHook,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    info!(
        target: "nosd",
        "job listener bind={bind} node={} noise=XX",
        identity.node_id()
    );
    serve_on(listener, identity, pinned, exec, crdt, skill).await
}

/// Igual a `serve`, mas com listener j? aberto (testes / socket activation).
pub async fn serve_on(
    listener: TcpListener,
    identity: Identity,
    pinned: Arc<Pinned>,
    exec: Executor,
    crdt: CrdtHook,
    skill: SkillHook,
) -> std::io::Result<()> {
    loop {
        let (sock, peer) = listener.accept().await?;
        let id = identity.clone();
        let pins = pinned.clone();
        let exec = exec.clone();
        let crdt = crdt.clone();
        let skill = skill.clone();
        tokio::spawn(async move {
            let mut s = match Secure::responder(sock, &id, &pins).await {
                Ok(s) => s,
                Err(e) => {
                    warn!(target: "nosd", "job noise handshake {peer}: {e}");
                    return;
                }
            };
            let req = match s.recv().await {
                Ok(b) => b,
                Err(e) => {
                    warn!(target: "nosd", "job recv: {e}");
                    return;
                }
            };
            match handle(&req, &id, &pins, &exec, &crdt, &skill).await {
                Ok(out) => {
                    if let Err(e) = s.send(&out).await {
                        warn!(target: "nosd", "job send: {e}");
                    }
                }
                Err(e) => warn!(target: "nosd", "job handle: {e}"),
            }
        });
    }
}

async fn handle(
    buf: &[u8],
    identity: &Identity,
    pinned: &Pinned,
    exec: &Executor,
    crdt: &CrdtHook,
    skill: &SkillHook,
) -> Result<Vec<u8>, String> {
    let pf = parse_frame(buf)?;
    let env: JobEnvelope = serde_json::from_slice(&pf.body).map_err(|e| format!("json: {e}"))?;
    if env.prompt.len() > max_prompt_for(env.kind) {
        return Err("prompt acima do limite".into());
    }
    if short_id(&env.key) != env.origin {
        return Err("id nao corresponde a chave".into());
    }
    match pinned.check_key(env.origin, &env.key) {
        Ok(()) => {}
        Err(PinError::Replay) => return Err("replay".into()),
        Err(e) => return Err(format!("peer nao autorizado: {e:?}")),
    }
    if !crate::ident::verify(&env.key, &pf.signed, &pf.sig) {
        return Err("assinatura invalida".into());
    }

    let t0 = Instant::now();
    let deadline = Duration::from_millis(env.deadline_ms.clamp(100, 120_000));
    let reply = if t0.elapsed() > deadline {
        Err("deadline excedido".into())
    } else if env.kind == KIND_CRDT {
        crdt(&env.prompt)
    } else if env.kind == KIND_SKILL {
        skill(&env.prompt)
    } else {
        exec(&env.prompt, env.budget_tokens)
    };
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let res = match reply {
        Ok(text) => JobResult {
            job_id: env.job_id,
            ok: true,
            text,
            error: None,
            worker: identity.node_id(),
            key: identity.public_key(),
            elapsed_ms,
        },
        Err(e) => JobResult {
            job_id: env.job_id,
            ok: false,
            text: String::new(),
            error: Some(e),
            worker: identity.node_id(),
            key: identity.public_key(),
            elapsed_ms,
        },
    };
    let body = serde_json::to_vec(&res).map_err(|e| e.to_string())?;
    info!(
        target: "nosd",
        "job done origin={} ok={} ms={elapsed_ms}",
        env.origin,
        res.ok
    );
    Ok(frame(&body, identity))
}

/// Cliente: envia o job pelo canal Noise e verifica a assinatura do worker.
pub async fn submit(
    addr: &str,
    identity: &Identity,
    pinned: &Arc<Pinned>,
    prompt: &str,
    deadline_ms: u64,
    budget_tokens: u32,
) -> Result<JobResult, String> {
    submit_kind(
        addr,
        identity,
        pinned,
        prompt,
        deadline_ms,
        budget_tokens,
        KIND_CHAT,
    )
    .await
}

/// M8: troca de estado (anti-entropy) pelo mesmo link autenticado.
pub async fn submit_crdt(
    addr: &str,
    identity: &Identity,
    pinned: &Arc<Pinned>,
    payload: &str,
    deadline_ms: u64,
) -> Result<JobResult, String> {
    submit_kind(addr, identity, pinned, payload, deadline_ms, 0, KIND_CRDT).await
}

/// M9.1: empurra um módulo `.wasm` para um peer (activa só se o hash bater).
pub async fn submit_skill(
    addr: &str,
    identity: &Identity,
    pinned: &Arc<Pinned>,
    name: &str,
    bytes: &[u8],
    deadline_ms: u64,
    extra: &serde_json::Value,
) -> Result<JobResult, String> {
    use base64::Engine as _;
    let hash = crate::content_hash(bytes);
    // Limite do módulo (MAX_SKILL_BYTES) e do corpo: base64 cresce ~4/3 e o
    // envelope de skill tem teto próprio (MAX_SKILL_PAYLOAD, T0.1).
    if bytes.len() > MAX_SKILL_BYTES {
        return Err("modulo acima de 512 KiB para push".into());
    }
    let mut payload = serde_json::json!({
        "name": name,
        "hash": hash,
        "wasm_b64": base64::engine::general_purpose::STANDARD.encode(bytes),
    });
    if let Some(obj) = extra.as_object() {
        for (k, v) in obj {
            payload[k] = v.clone();
        }
    }
    submit_kind(
        addr,
        identity,
        pinned,
        &payload.to_string(),
        deadline_ms,
        0,
        KIND_SKILL,
    )
    .await
}

async fn submit_kind(
    addr: &str,
    identity: &Identity,
    pinned: &Arc<Pinned>,
    prompt: &str,
    deadline_ms: u64,
    budget_tokens: u32,
    kind: u8,
) -> Result<JobResult, String> {
    if prompt.len() > max_prompt_for(kind) {
        return Err("prompt acima do limite".into());
    }
    let env = JobEnvelope {
        job_id: new_job_id(),
        kind,
        prompt: prompt.to_string(),
        deadline_ms: deadline_ms.clamp(100, 120_000),
        budget_tokens,
        origin: identity.node_id(),
        key: identity.public_key(),
    };
    let body = serde_json::to_vec(&env).map_err(|e| e.to_string())?;
    let out = frame(&body, identity);

    let budget = Duration::from_millis(deadline_ms.clamp(100, 120_000) + 2_000);
    let sock = tokio::time::timeout(budget, TcpStream::connect(addr))
        .await
        .map_err(|_| "connect timeout".to_string())?
        .map_err(|e| format!("connect: {e}"))?;
    let mut s = Secure::initiator(sock, identity, pinned).await?;
    s.send(&out).await?;
    let buf = tokio::time::timeout(budget, s.recv())
        .await
        .map_err(|_| "read timeout".to_string())??;

    let pf = parse_frame(&buf)?;
    let res: JobResult = serde_json::from_slice(&pf.body).map_err(|e| format!("json: {e}"))?;
    if short_id(&res.key) != res.worker {
        return Err("worker id nao corresponde a chave".into());
    }
    if !crate::ident::verify(&res.key, &pf.signed, &pf.sig) {
        return Err("assinatura do worker invalida".into());
    }
    pinned
        .check_key(res.worker, &res.key)
        .map_err(|e| format!("worker nao autorizado: {e:?}"))?;
    Ok(res)
}

/// (mantido para compat de leitura; o canal agora é sempre Noise)
pub async fn shutdown(_sock: &mut TcpStream) {
    let _ = _sock.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ident::{Identity, Pinned};
    use std::collections::HashSet;

    fn ident(seed: u8) -> Identity {
        Identity::from_seed([seed; 32], HashSet::new(), true)
    }

    #[tokio::test]
    async fn noise_job_roundtrip() {
        let server = ident(11);
        let client = ident(12);
        let pins_srv = Arc::new(Pinned::new());
        let pins_cli = Arc::new(Pinned::new());
        let exec: Executor = Arc::new(|p: &str, _b: u32| Ok(format!("pong:{p}")));
        let crdt: CrdtHook = Arc::new(|p: &str| Ok(p.to_string()));

        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let s = server.clone();
        let pins_task = pins_srv.clone();
        tokio::spawn(async move {
            let skill: SkillHook = Arc::new(|_p: &str| Err("skill hook nao usado".into()));
            let _ = serve_on(l, s, pins_task, exec, crdt, skill).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let r = submit(&addr, &client, &pins_cli, "oi", 5_000, 16)
            .await
            .expect("job ok");
        assert!(r.ok, "{r:?}");
        assert_eq!(r.text, "pong:oi");
        assert_eq!(r.worker, server.node_id());
        assert_eq!(pins_cli.peer_count(), 1, "cliente fixou o worker");
        assert_eq!(pins_srv.peer_count(), 1, "servidor fixou o cliente");
    }

    /// T0.1: push de módulo grande tem de atravessar o link (o limite de
    /// 8 KiB de chat não se aplica a KIND_SKILL) e o teto de 512 KiB mantém-se.
    #[tokio::test]
    async fn skill_push_aceita_modulo_grande_e_recusa_acima_do_cap() {
        let server = ident(31);
        let client = ident(32);
        let pins_srv = Arc::new(Pinned::new());
        let pins_cli = Arc::new(Pinned::new());
        let exec: Executor = Arc::new(|p: &str, _b: u32| Ok(p.to_string()));
        let crdt: CrdtHook = Arc::new(|p: &str| Ok(p.to_string()));

        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let s = server.clone();
        let pins_task = pins_srv.clone();
        tokio::spawn(async move {
            let skill: SkillHook = Arc::new(|p: &str| {
                let v: serde_json::Value = serde_json::from_str(p).map_err(|e| e.to_string())?;
                let b = v["wasm_b64"].as_str().ok_or("sem wasm_b64")?;
                if b.len() < 100_000 {
                    return Err("modulo nao chegou inteiro".into());
                }
                Ok("skill grande ok".into())
            });
            let _ = serve_on(l, s, pins_task, exec, crdt, skill).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let big = vec![0x00u8; 300 * 1024];
        let r = submit_skill(
            &addr,
            &client,
            &pins_cli,
            "big",
            &big,
            10_000,
            &serde_json::json!({}),
        )
        .await
        .expect("push de 300 KiB deve atravessar o link");
        assert!(r.ok, "{r:?}");
        assert_eq!(r.text, "skill grande ok");

        let too_big = vec![0u8; MAX_SKILL_BYTES + 1];
        assert!(
            submit_skill(
                &addr,
                &client,
                &pins_cli,
                "big",
                &too_big,
                10_000,
                &serde_json::json!({}),
            )
            .await
            .is_err(),
            "acima de 512 KiB recusa no emissor"
        );
    }
}
