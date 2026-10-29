//! V5 (plano vampiro) — MCP mínimo, **fail-closed por tool**.
//!
//! Origem da spec: `openjarvis/mcp/{protocol,transport,server}.py` e
//! `openjarvis/tools/mcp_adapter.py` (o crate Rust deles tem ~8 KiB e não
//! serve). Aqui a fronteira é o `CapGate`: o registry **não** executa nada
//! sozinho — delega no `ToolExecutor` do daemon e exige as caps declaradas
//! *antes* da chamada.
//!
//! Estado honesto (claim IMPLEMENTED, não VERIFIED):
//! - transporte JSON-RPC **stdio** (`stdio::serve_stdio`) e **in-process**
//!   (`handle_jsonrpc`) implementados;
//! - HTTP/SSE, `resources/*` e `prompts/*` **não** implementados;
//! - tool desconhecida = deny; tool sem caps = deny; side-effect exige HITL.

#![forbid(unsafe_code)]

pub mod stdio;

use nos_core::{CapBits, CapGate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Versão de protocolo que anunciamos (data-version do spec MCP).
pub const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("cap denied: {0}")]
    Cap(String),
    #[error("hitl required for side-effect tool {0}")]
    HitlRequired(String),
    #[error("invalid params: {0}")]
    BadParams(String),
    #[error("executor: {0}")]
    Exec(String),
}

impl McpError {
    /// Código JSON-RPC (faixa de aplicação: -32000..-32099).
    pub fn code(&self) -> i64 {
        match self {
            Self::UnknownTool(_) => -32001,
            Self::Cap(_) => -32003,
            Self::HitlRequired(_) => -32004,
            Self::BadParams(_) => -32602,
            Self::Exec(_) => -32603,
        }
    }
}

/// Declaração de uma tool. `required` é o **piso** de capacidades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub required: CapBits,
    /// Efeito colateral (state/FS/NET/infer): exige HITL quando `hitl_kind`
    /// estiver definido.
    pub side_effect: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitl_kind: Option<String>,
}

impl ToolSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>, required: CapBits) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            required,
            side_effect: false,
            hitl_kind: None,
        }
    }

    pub fn side_effect(mut self, hitl_kind: impl Into<String>) -> Self {
        self.side_effect = true;
        self.hitl_kind = Some(hitl_kind.into());
        self
    }
}

/// Vista pública (o que o `tools/list` devolve a este peer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolView {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    pub side_effect: bool,
}

#[derive(Debug, Default)]
pub struct Registry {
    tools: Vec<ToolSpec>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, spec: ToolSpec) -> Result<(), McpError> {
        if self.tools.iter().any(|t| t.name == spec.name) {
            return Err(McpError::BadParams(format!("duplicate tool {}", spec.name)));
        }
        if spec.name.trim().is_empty() {
            return Err(McpError::BadParams("empty tool name".into()));
        }
        self.tools.push(spec);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn get(&self, name: &str) -> Option<&ToolSpec> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// `tools/list`: **filtra** pelo que o peer pode realmente invocar.
    pub fn list(&self, gate: &CapGate, gen: u64) -> Vec<ToolView> {
        self.tools
            .iter()
            .filter(|t| can(gate, gen, t.required))
            .map(|t| ToolView {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: json!({ "type": "object", "additionalProperties": true }),
                side_effect: t.side_effect,
            })
            .collect()
    }
}

/// Executa uma tool já autorizada (implementado pelo daemon).
pub trait ToolExecutor {
    fn call(&self, name: &str, args: &Value) -> Result<Value, String>;
}

fn can(gate: &CapGate, gen: u64, bits: CapBits) -> bool {
    bits.0 == 0 || gate.allow(bits, gen).is_ok()
}

/// Chamada gated: caps do peer → HITL (se side-effect) → execução.
pub fn call_tool(
    reg: &Registry,
    exec: &dyn ToolExecutor,
    gate: &CapGate,
    gen: u64,
    hitl_ok: bool,
    name: &str,
    args: &Value,
) -> Result<Value, McpError> {
    let spec = reg
        .get(name)
        .ok_or_else(|| McpError::UnknownTool(name.into()))?;
    if !can(gate, gen, spec.required) {
        return Err(McpError::Cap(format!(
            "tool {name} requires caps=0x{:x}",
            spec.required.0
        )));
    }
    if spec.side_effect && spec.hitl_kind.is_some() && !hitl_ok {
        return Err(McpError::HitlRequired(name.into()));
    }
    exec.call(name, args).map_err(McpError::Exec)
}

/// `initialize` (handshake; não revela tools sem caps).
pub fn initialize() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "nosd-mcp",
            "version": env!("CARGO_PKG_VERSION"),
            "honesty": "stdio + in-process; resources/prompts NOT_DONE",
        },
    })
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_err(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Handler JSON-RPC 2.0 (uma mensagem → uma resposta; notificações = `None`).
#[allow(clippy::too_many_arguments)]
pub fn handle_jsonrpc(
    reg: &Registry,
    exec: &dyn ToolExecutor,
    gate: &CapGate,
    gen: u64,
    hitl_ok: bool,
    msg: &Value,
) -> Option<Value> {
    let method = msg.get("method")?.as_str()?.to_string();
    let id = msg.get("id").cloned();
    // Notificação (sem id) não tem resposta.
    let id = id?;
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));

    let out = match method.as_str() {
        "initialize" => rpc_ok(id, initialize()),
        "ping" => rpc_ok(id, json!({})),
        "tools/list" => rpc_ok(id, json!({ "tools": reg.list(gate, gen) })),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(reg, exec, gate, gen, hitl_ok, name, &args) {
                Ok(v) => rpc_ok(
                    id,
                    json!({ "content": [{ "type": "text", "text": v.to_string() }] }),
                ),
                Err(e) => rpc_err(id, e.code(), e.to_string()),
            }
        }
        other => rpc_err(id, -32601, format!("method not found: {other}")),
    };
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl ToolExecutor for Echo {
        fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
            Ok(json!({ "tool": name, "args": args }))
        }
    }

    fn reg() -> Registry {
        let mut r = Registry::new();
        r.register(ToolSpec::new(
            "nosd.health",
            "health read-only",
            CapBits::NONE,
        ))
        .unwrap();
        r.register(ToolSpec::new("nosd.infer", "infer", CapBits::INFER).side_effect("act"))
            .unwrap();
        r.register(ToolSpec::new("nosd.root", "root", CapBits::NIX_WRITE).side_effect("nix.apply"))
            .unwrap();
        r
    }

    fn gate(caps: CapBits) -> CapGate {
        CapGate::new(caps, 1)
    }

    #[test]
    fn list_filters_by_capability() {
        let r = reg();
        let g = gate(CapBits::NONE);
        let names: Vec<_> = r.list(&g, 1).into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["nosd.health"]);

        let g = gate(CapBits::INFER);
        let names: Vec<_> = r.list(&g, 1).into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["nosd.health", "nosd.infer"]);
    }

    #[test]
    fn unknown_tool_is_denied() {
        let r = reg();
        let e =
            call_tool(&r, &Echo, &gate(CapBits::FULL), 1, true, "nope", &json!({})).unwrap_err();
        assert!(matches!(e, McpError::UnknownTool(_)));
        assert_eq!(e.code(), -32001);
    }

    #[test]
    fn missing_cap_is_denied_even_if_registered() {
        let r = reg();
        let e = call_tool(
            &r,
            &Echo,
            &gate(CapBits::NONE),
            1,
            true,
            "nosd.infer",
            &json!({}),
        )
        .unwrap_err();
        assert!(matches!(e, McpError::Cap(_)));
        assert_eq!(e.code(), -32003);
    }

    #[test]
    fn side_effect_requires_hitl() {
        let r = reg();
        let e = call_tool(
            &r,
            &Echo,
            &gate(CapBits::NIX_WRITE),
            1,
            false,
            "nosd.root",
            &json!({}),
        )
        .unwrap_err();
        assert!(matches!(e, McpError::HitlRequired(_)));

        let ok = call_tool(
            &r,
            &Echo,
            &gate(CapBits::NIX_WRITE),
            1,
            true,
            "nosd.root",
            &json!({}),
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn duplicate_registration_fails_closed() {
        let mut r = reg();
        let e = r
            .register(ToolSpec::new("nosd.health", "dup", CapBits::NONE))
            .unwrap_err();
        assert!(matches!(e, McpError::BadParams(_)));
    }

    #[test]
    fn jsonrpc_roundtrip_initialize_list_call() {
        let r = reg();
        let g = gate(CapBits::INFER);
        let init = handle_jsonrpc(
            &r,
            &Echo,
            &g,
            1,
            false,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);

        let list = handle_jsonrpc(
            &r,
            &Echo,
            &g,
            1,
            false,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 2);

        let call = handle_jsonrpc(
            &r,
            &Echo,
            &g,
            1,
            false,
            &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nosd.health","arguments":{}}}),
        )
        .unwrap();
        assert!(call["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("nosd.health"));

        let denied = handle_jsonrpc(
            &r,
            &Echo,
            &g,
            1,
            false,
            &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nosd.root"}}),
        )
        .unwrap();
        assert_eq!(denied["error"]["code"], -32003);
    }

    #[test]
    fn unknown_method_and_notifications() {
        let r = reg();
        let g = gate(CapBits::NONE);
        let out = handle_jsonrpc(
            &r,
            &Echo,
            &g,
            1,
            false,
            &json!({"jsonrpc":"2.0","id":9,"method":"resources/list"}),
        )
        .unwrap();
        assert_eq!(out["error"]["code"], -32601);

        let none = handle_jsonrpc(
            &r,
            &Echo,
            &g,
            1,
            false,
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        );
        assert!(none.is_none());
    }
}
