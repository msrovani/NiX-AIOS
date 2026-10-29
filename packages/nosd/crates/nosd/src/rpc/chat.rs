//! RPC handlers — fatia 0.5.2 (+ V1/V2 do plano vampiro: energia/custo por turno).
use super::*;
use nos_core::{CapGate, CostModel};
use nos_ipc::{Request, Response};
use serde_json::json;

/// V1/V2 — fecha a janela de energia de um turno e calcula o contrafactual.
///
/// `tokens` são **estimativa** (`chars/4`) e vão rotulados como tal; o custo
/// de nuvem leva sempre `ref_date` + `ref_model` + `HYPOTHESIS` (ADR-0113).
fn turn_block(state: &AppState, prompt_chars: usize, reply_chars: usize) -> serde_json::Value {
    let delta = state.energy.lock().sample();
    let cost = CostModel::from_env();
    let tokens_in = nos_core::tokens_estimate(prompt_chars);
    let tokens_out = nos_core::tokens_estimate(reply_chars);
    json!({
        "joules": delta.joules,
        "source": delta.source,
        "scope": delta.scope,
        "joules_total": delta.joules_total,
        "ts_ms": delta.ts_ms,
        "cost": {
            "local_eur": cost.local_eur(delta.joules),
            "cloud_usd": cost.cloud_usd(tokens_in, tokens_out),
            "tokens": {
                "est_in": tokens_in,
                "est_out": tokens_out,
                "source": "char_heuristic",
            },
            "honesty": cost.honesty_tag(),
        },
    })
}

fn attach_turn(mut resp: Response, block: serde_json::Value) -> Response {
    if resp.ok {
        if let Some(obj) = resp.result.as_object_mut() {
            obj.insert("turn".into(), block);
        }
    }
    resp
}

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "chat.submit" => {
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // V1: base da janela (sysfs; sem spawn de nvidia-smi).
            state.energy.lock().prime();
            let out = state.hermes.chat_submit(gate, prompt);
            let (ok, error, result) = match out {
                Ok(v) => (true, None, v),
                Err(e) => (false, Some(e.to_string()), json!(null)),
            };
            let reply_chars = result
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::len)
                .unwrap_or(0);
            let resp = Response {
                id: req.id,
                ok,
                error,
                result,
            };
            // S2 assíncrono não tem resposta final: não inventar joules.
            if resp.result.get("ready").and_then(|v| v.as_bool()) == Some(true) {
                let block = turn_block(state, prompt.len(), reply_chars);
                attach_turn(resp, block)
            } else {
                resp
            }
        }
        "chat.poll" => {
            let job_id = req.params.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
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
            state.energy.lock().prime();
            match state.hermes.chat(gate, prompt) {
                Ok(text) => {
                    let block = turn_block(state, prompt.len(), text.len());
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({ "text": text, "turn": block }),
                    }
                }
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
