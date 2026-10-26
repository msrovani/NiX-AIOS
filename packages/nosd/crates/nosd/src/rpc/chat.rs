//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "chat.submit" => {
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match state.hermes.chat_submit(&gate, prompt) {
                Ok(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: v,
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
        "chat.poll" => {
            let job_id = req
                .params
                .get("id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            match state.hermes.chat_poll(job_id) {
                Some(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: v,
                },
                None => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "id": job_id, "pending": true }),
                },
            }
        }
        "memory.recall" => {
            let q = req
                .params
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match state.hermes.recall_context(q) {
                Ok(texts) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!(texts),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }


        "chat" | "infer.submit" => {
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match state.hermes.chat(&gate, prompt) {
                Ok(text) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "text": text }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }

        _ => return None,
    })
}
