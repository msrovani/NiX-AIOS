//! M7 — Jobs remotos (NoProto sobre TCP) com frames assinados.
//!
//! V1 (esta fatia): o cliente envia um envelope, o worker executa **localmente**
//! e devolve um resultado assinado. Não há confidencialidade (Noise fica em
//! M6.1) nem scheduler distribuído — o cliente escolhe o endereço.
//!
//! Wire: `NJOB | ver u8 | len u32 LE | body | sig 64`
//! A assinatura cobre `NJOB|ver|len|body`. O recetor verifica contra a chave
//! **fixada** no mesh (TOFU) — um par desconhecido não entra.

use crate::ident::{short_id, Identity, PinError, Pinned};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

pub const MAGIC: &[u8] = b"NJOB";
pub const VER: u8 = 1;
pub const MAX_BODY: usize = 1 << 20; // 1 MiB por job (prompt + overhead)
pub const MAX_PROMPT: usize = 8 * 1024;
pub const DEFAULT_DEADLINE_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnvelope {
    pub job_id: [u8; 16],
    /// 1 = chat (v1).
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
    /// Id derivado da chave do worker.
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

/// Executor local (prompt, budget) -> texto.
pub type Executor = Arc<dyn Fn(&str, u32) -> Result<String, String> + Send + Sync>;

/// M8: hook de estado — recebe o payload e devolve o snapshot já merged.
pub type CrdtHook = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// `kind` do envelope.
pub const KIND_CHAT: u8 = 1;
pub const KIND_CRDT: u8 = 2;

/// Servidor de jobs. `exec` corre o trabalho local (chat, skill, ...).
pub async fn serve(
    bind: &str,
    identity: Identity,
    pinned: Arc<Pinned>,
    exec: Executor,
    crdt: CrdtHook,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    info!(target: "nosd", "job listener bind={bind} node={}", identity.node_id());
    loop {
        let (mut sock, peer) = listener.accept().await?;
        let id = identity.clone();
        let pins = pinned.clone();
        let exec = exec.clone();
        let crdt = crdt.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(&mut sock, &id, &pins, &exec, &crdt).await {
                warn!(target: "nosd", "job conn {peer} err: {e}");
            }
        });
    }
}

async fn handle(
    sock: &mut TcpStream,
    identity: &Identity,
    pinned: &Pinned,
    exec: &Executor,
    crdt: &CrdtHook,
) -> Result<(), String> {
    let mut head = [0u8; 9];
    sock.read_exact(&mut head)
        .await
        .map_err(|e| e.to_string())?;
    if &head[0..4] != MAGIC || head[4] != VER {
        return Err("header invalido".into());
    }
    let len = u32::from_le_bytes(head[5..9].try_into().unwrap()) as usize;
    if len > MAX_BODY {
        return Err("job demasiado grande".into());
    }
    let mut rest = vec![0u8; len + 64];
    sock.read_exact(&mut rest)
        .await
        .map_err(|e| e.to_string())?;
    let body = &rest[..len];
    let sig = &rest[len..];

    let env: JobEnvelope = serde_json::from_slice(body).map_err(|e| format!("json: {e}"))?;
    if env.prompt.len() > MAX_PROMPT {
        return Err("prompt acima do limite".into());
    }
    if short_id(&env.key) != env.origin {
        return Err("id nao corresponde a chave".into());
    }
    // Autenticidade: chave fixada no mesh.
    match pinned.check_key(env.origin, &env.key) {
        Ok(()) => {}
        Err(PinError::Replay) => return Err("replay".into()),
        Err(e) => return Err(format!("peer nao autorizado: {e:?}")),
    }
    let mut signed = head.to_vec();
    signed.extend_from_slice(body);
    if !crate::ident::verify(&env.key, &signed, sig) {
        return Err("assinatura invalida".into());
    }

    let t0 = Instant::now();
    let deadline = Duration::from_millis(env.deadline_ms.clamp(100, 120_000));
    let reply = if t0.elapsed() > deadline {
        Err("deadline excedido".into())
    } else if env.kind == KIND_CRDT {
        crdt(&env.prompt)
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
    let out = frame(&body, identity);
    sock.write_all(&out).await.map_err(|e| e.to_string())?;
    sock.shutdown().await.ok();
    info!(
        target: "nosd",
        "job done origin={} ok={} ms={elapsed_ms}",
        env.origin,
        res.ok
    );
    Ok(())
}

/// Cliente: envia o job e verifica a assinatura do worker.
pub async fn submit(
    addr: &str,
    identity: &Identity,
    pinned: &Pinned,
    prompt: &str,
    deadline_ms: u64,
    budget_tokens: u32,
) -> Result<JobResult, String> {
    if prompt.len() > MAX_PROMPT {
        return Err("prompt acima do limite".into());
    }
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
    pinned: &Pinned,
    payload: &str,
    deadline_ms: u64,
) -> Result<JobResult, String> {
    submit_kind(addr, identity, pinned, payload, deadline_ms, 0, KIND_CRDT).await
}

async fn submit_kind(
    addr: &str,
    identity: &Identity,
    pinned: &Pinned,
    prompt: &str,
    deadline_ms: u64,
    budget_tokens: u32,
    kind: u8,
) -> Result<JobResult, String> {
    if prompt.len() > MAX_PROMPT {
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
    let _ = budget_tokens;

    let mut sock = tokio::time::timeout(
        Duration::from_millis(deadline_ms.clamp(100, 120_000) + 2_000),
        TcpStream::connect(addr),
    )
    .await
    .map_err(|_| "connect timeout".to_string())?
    .map_err(|e| format!("connect: {e}"))?;
    sock.write_all(&out).await.map_err(|e| e.to_string())?;
    sock.shutdown().await.ok();

    let mut buf = Vec::new();
    let read = tokio::time::timeout(
        Duration::from_millis(deadline_ms.clamp(100, 120_000) + 2_000),
        sock.read_to_end(&mut buf),
    )
    .await
    .map_err(|_| "read timeout".to_string())?
    .map_err(|e| e.to_string())?;
    let _ = read;

    let pf = parse_frame(&buf)?;
    let res: JobResult = serde_json::from_slice(&pf.body).map_err(|e| format!("json: {e}"))?;
    if short_id(&res.key) != res.worker {
        return Err("worker id nao corresponde a chave".into());
    }
    if !crate::ident::verify(&res.key, &pf.signed, &pf.sig) {
        return Err("assinatura do worker invalida".into());
    }
    // Se o worker já estava fixado, a chave tem de ser a mesma.
    if let Err(e) = pinned.check_key(res.worker, &res.key) {
        return Err(format!("worker nao autorizado: {e:?}"));
    }
    Ok(res)
}
