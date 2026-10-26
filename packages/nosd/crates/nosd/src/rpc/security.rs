//! RPC de segurança — rotação de token/geração (S-03).
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "security.rotate" => match state.rotate_ipc() {
            Ok((token, generation)) => {
                let _ =
                    state
                        .hermes
                        .record_opar("security", "rotate", "ipc_token", "rotated", "ok");
                Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "rotated": true,
                        "generation": generation,
                        "token": token,
                        "token_file": state.ipc_token_file.display().to_string(),
                        "note": "old TCP/HTTP tokens are revoked; Unix peers use SO_PEERCRED",
                    }),
                }
            }
            Err(e) => Response {
                id: req.id,
                ok: false,
                error: Some(format!("token rotation failed: {e}")),
                result: json!(null),
            },
        },
        _ => return None,
    })
}
