//! V5 (plano vampiro) — superfície RPC do MCP mínimo.
//!
//! O registry é **server-authored** (não vem do cliente) e só contém tools
//! read-only. `mcp.call` aplica as caps **da tool** com o `CapGate` do peer;
//! uma tool de side-effect exigiria HITL (`hitl_ok=false` aqui de propósito).
//!
//! Honestidade: transporte JSON-RPC em processo; `stdio`/SSE **NOT_DONE**.

use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use nos_mcp::{Registry, ToolExecutor, ToolSpec};
use serde_json::json;

struct NosdTools<'a> {
    state: &'a AppState,
}

impl ToolExecutor for NosdTools<'_> {
    fn call(&self, name: &str, _args: &serde_json::Value) -> Result<serde_json::Value, String> {
        match name {
            "nosd.health" => Ok(json!({
                "status": "ok",
                "engine": self.state.hermes.hotswap.engine_name(),
                "hub_loaded": self.state.hermes.hub.loaded_count(),
                "mesh_peers": self.state.mesh.peer_count(),
            })),
            "nosd.score" => Ok(serde_json::to_value(super::build_runtime_score(
                self.state, "runtime", "mcp",
            ))
            .unwrap_or(json!(null))),
            other => Err(format!("tool not implemented: {other}")),
        }
    }
}

/// Registry mínimo e determinístico (duas tools read-only, caps NONE).
pub(crate) fn registry() -> Registry {
    let mut reg = Registry::new();
    let _ = reg.register(ToolSpec::new(
        "nosd.health",
        "estado do daemon (read-only)",
        nos_core::CapBits::NONE,
    ));
    let _ = reg.register(ToolSpec::new(
        "nosd.score",
        "score honesto do runtime (read-only)",
        nos_core::CapBits::NONE,
    ));
    reg
}

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "mcp.list" => {
            let reg = registry();
            let tools = reg.list(gate, state.ipc_generation());
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "count": tools.len(),
                    "tools": tools,
                    "transport": "in-process",
                    "honesty": "aqui in-process; stdio read-only via `nosmcp`; resources/prompts NOT_DONE",
                }),
            }
        }
        "mcp.call" => {
            let tool = req
                .params
                .get("tool")
                .or_else(|| req.params.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req
                .params
                .get("arguments")
                .or_else(|| req.params.get("args"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            let reg = registry();
            let exec = NosdTools { state };
            // HITL não é aceito aqui: tools de side-effect são recusadas até
            // existir um caminho de grant explícito por tool.
            match nos_mcp::call_tool(
                &reg,
                &exec,
                gate,
                state.ipc_generation(),
                false,
                tool,
                &args,
            ) {
                Ok(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "tool": tool, "content": v }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "code": e.code() }),
                },
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_only_read_only_tools() {
        let reg = registry();
        assert_eq!(reg.len(), 2);
        for name in ["nosd.health", "nosd.score"] {
            let spec = reg.get(name).expect("tool registada");
            assert_eq!(spec.required, nos_core::CapBits::NONE);
            assert!(!spec.side_effect);
            assert!(spec.hitl_kind.is_none());
        }
    }

    #[test]
    fn registry_lists_both_for_any_profile() {
        let reg = registry();
        let g = CapGate::new(nos_core::CapBits::NONE, 1);
        assert_eq!(reg.list(&g, 1).len(), 2);
    }
}
