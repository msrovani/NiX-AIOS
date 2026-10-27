//! Framing length-prefix + protocolo CapGate sobre Unix socket / TCP loopback.

#![forbid(unsafe_code)]

use nos_core::CapBits;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_SOCKET: &str = "/run/nosd/nosd.sock";

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("short read")]
    Short,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame too large")]
    TooLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    /// Deprecated: ignorado pelo daemon (I.1). Mantido por compatibilidade de wire.
    #[serde(default)]
    pub caps: u32,
    /// Deprecated: geração é server-authored; mantido por compatibilidade.
    #[serde(default)]
    pub cap_gen: u64,
    /// Hint de cliente para logs/auditoria. NUNCA define permissão.
    #[serde(default)]
    pub agent: String,
    /// Token de transporte para TCP/HTTP. Unix usa `SO_PEERCRED`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub result: serde_json::Value,
}

impl Request {
    pub fn required_cap(&self) -> CapBits {
        match self.method.as_str() {
            "health" | "score" | "fleet.health" | "model.list" | "model.suggest" | "model.plan"
            | "route.explain" | "cognitive.tick" | "artifact.validate" | "ui.list" | "ui.card"
            | "wifi.scan" | "voice.status" | "transmute.status" | "orb.get" | "orb.set"
            | "orb.sense" => CapBits::NONE,
            "infer.submit" | "chat" | "chat.submit" | "chat.poll" | "model.unload"
            | "voice.stt" | "voice.tts" | "voice.turn" => CapBits::INFER,
            "model.load" => CapBits::INFER.union(CapBits::FS),
            "model.fetch" | "model.search" | "firmware.fetch" => CapBits::NET.union(CapBits::FS),
            "blob.deliberate" => CapBits::NONE,
            "transmute.run" => CapBits::NET.union(CapBits::FS).union(CapBits::NIX_WRITE),
            "agent.spawn" | "factory.promote" | "factory.seek" | "factory.synthesize"
            | "store.install" | "store.search" | "store.index" => CapBits::AGENT_SPAWN,
            "memory.remember" | "memory.recall" | "memory.curate" => CapBits::MEMORY,
            "nix.propose" | "nix.apply" | "nix.rollback" | "nix.confirm" => CapBits::NIX_WRITE,
            "hw.act" | "audio.act" | "audio.volume" | "gpu.plan" | "observe.snapshot"
            | "policy.matrix" | "device.recipe" => CapBits::OBSERVE,
            "self.heal.evaluate" => CapBits::NIX_WRITE.union(CapBits::HITL),
            // Rotação de token TCP/HTTP: só admin (LocalAdmin).
            "security.rotate" => CapBits::NIX_WRITE.union(CapBits::HITL),
            "systemctl.scoped" => CapBits::HITL,
            "nix.build_vm" => CapBits::NIX_WRITE,
            "wifi.connect" => CapBits::HITL,
            // Abrir/fechar janela = livre; submeter HITL = CapBits::HITL
            "ui.open" | "ui.close" => CapBits::NONE,
            "ui.action" => CapBits::HITL,
            _ => CapBits::DENY_UNKNOWN,
        }
    }
}

pub fn encode(msg: &impl Serialize) -> Result<Vec<u8>, IpcError> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > 16 * 1024 * 1024 {
        return Err(IpcError::TooLarge);
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, IpcError> {
    Ok(serde_json::from_slice(body)?)
}

/// Extrai um frame completo de um buffer acumulado; devolve (msg_body, rest).
pub fn take_frame(buf: &[u8]) -> Result<Option<(Vec<u8>, usize)>, IpcError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let n = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    if n > 16 * 1024 * 1024 {
        return Err(IpcError::TooLarge);
    }
    if buf.len() < 4 + n {
        return Ok(None);
    }
    Ok(Some((buf[4..4 + n].to_vec(), 4 + n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_frame() {
        let req = Request {
            id: 1,
            method: "health".into(),
            caps: 0,
            cap_gen: 0,
            agent: "nosctl".into(),
            token: None,
            params: serde_json::json!({}),
        };
        let enc = encode(&req).unwrap();
        let (body, consumed) = take_frame(&enc).unwrap().unwrap();
        assert_eq!(consumed, enc.len());
        let back: Request = decode_body(&body).unwrap();
        assert_eq!(back.method, "health");
    }

    #[test]
    fn rejects_oversized_frame() {
        // 0xFFFFFFFF (u32::MAX) nao pode ser aceite como tamanho de frame.
        let buf = [0xffu8, 0xff, 0xff, 0xff];
        assert!(matches!(take_frame(&buf), Err(IpcError::TooLarge)));
    }

    #[test]
    fn unknown_method_is_deny_unknown() {
        // Fail-closed: metodo nao listado nunca herda caps.
        let req = Request {
            id: 1,
            method: "nao.existe".into(),
            caps: 0,
            cap_gen: 0,
            agent: String::new(),
            token: None,
            params: serde_json::json!({}),
        };
        assert_eq!(req.required_cap(), CapBits::DENY_UNKNOWN);
    }
}
