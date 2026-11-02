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
    // V8.2: cobertura honesta — um turno sem fonte não é "0 J medidos".
    state
        .turns_total
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if matches!(delta.source.as_str(), "rapl" | "nvml" | "tdp_estimate") {
        state
            .turns_measured
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
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
                Err(e) => (
                    false,
                    Some(nos_core::trim_diagnostic(&e.to_string(), true)),
                    json!(null),
                ),
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
            let k = req.params.get("k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let scope = req.params.get("scope").and_then(|v| v.as_str());
            let mode = nos_memory::RecallMode::parse(
                req.params
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("lexical"),
            );
            match state.hermes.memory.recall_governed(nos_memory::RecallReq {
                query: q,
                k,
                scope,
                entities: &[],
                mode,
                embedding: None,
                path: None,
            }) {
                Ok(hits) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "api_version": nos_memory::MEMORY_API_VERSION,
                        "hits": hits,
                    }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(nos_core::trim_diagnostic(&e.to_string(), true)),
                    result: json!(null),
                },
            }
        }
        "memory.health" => {
            let view = req
                .params
                .get("view")
                .and_then(|v| v.as_str())
                .unwrap_or("era");
            let ok = state.hermes.memory.health();
            let detail = state.hermes.memory.health_view(view).ok();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "memory_ok": ok,
                    "backend": format!("{:?}", state.hermes.memory.backend()),
                    "api_version": nos_memory::MEMORY_API_VERSION,
                    "view": detail,
                }),
            }
        }
        "memory.remember" => {
            let text = req
                .params
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if text.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("text required".into()),
                    result: json!(null),
                });
            }
            let scope = req
                .params
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("aios/user")
                .to_string();
            let entities: Vec<String> = req
                .params
                .get("entities")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_else(|| vec!["mom/fact".into()]);
            let key = req
                .params
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("md/L3/user/{}", uuid::Uuid::new_v4()));
            // Two-pass: recall gather no mesmo turno antes do remember_user.
            let mut tok = state.hermes.memory.begin_turn();
            let _ = state.hermes.memory.recall_gov(
                &mut tok,
                nos_memory::RecallReq {
                    query: text,
                    k: 3,
                    scope: Some(scope.as_str()),
                    entities: &[],
                    mode: nos_memory::RecallMode::Lexical,
                    embedding: None,
                    path: None,
                },
            );
            match state.hermes.memory.remember_user(
                &tok,
                nos_memory::MemoryRecord {
                    key,
                    text: text.into(),
                    scope,
                    entities,
                },
            ) {
                Ok(sk) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "key": sk }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(nos_core::trim_diagnostic(&e.to_string(), true)),
                    result: json!(null),
                },
            }
        }
        "memory.curate" => {
            let op = req.params.get("op").and_then(|v| v.as_str()).unwrap_or("");
            let key = req.params.get("key").and_then(|v| v.as_str());
            match state.hermes.memory.curate(op, key) {
                Ok(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: v,
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(nos_core::trim_diagnostic(&e.to_string(), true)),
                    result: json!(null),
                },
            }
        }
        "memory.decide" => Response {
            id: req.id,
            ok: false,
            error: Some("unsupported: decide (host chooses; Hit != order)".into()),
            result: json!({ "unsupported": true }),
        },
        "memory.sync" => {
            let addr = req
                .params
                .get("addr")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if addr.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("memory.sync requer addr (peer mesh JOB_BIND)".into()),
                    result: json!(null),
                });
            }
            if !state.hermes.memory.telepathy_ready() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("telepathy unavailable".into()),
                    result: json!({ "telepathy": "unsupported" }),
                });
            }
            let Some(identity) = state.mesh.identity() else {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("sem identidade de no (mesh)".into()),
                    result: json!({ "need": "identity" }),
                });
            };
            let max = req.params.get("max").and_then(|v| v.as_u64()).unwrap_or(64) as usize;
            let local_export = match state.hermes.memory.telepathy_export(max) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(e.to_string()),
                        result: json!(null),
                    })
                }
            };
            let payload = json!({
                "op": "exchange",
                "records_b64": local_export,
                "max": max,
            })
            .to_string();
            let pinned = state.mesh.pinned();
            let id1 = identity.clone();
            let pin1 = pinned.clone();
            let a1 = addr.clone();
            let exchange = std::thread::spawn(move || -> Result<String, String> {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                rt.block_on(async move {
                    let r =
                        nos_mesh::job::submit_memory(&a1, &id1, &pin1, &payload, 60_000).await?;
                    if !r.ok {
                        return Err(r.error.unwrap_or_else(|| "memory.sync peer failed".into()));
                    }
                    Ok(r.text)
                })
            });
            let peer_text = match exchange.join() {
                Ok(Ok(t)) => t,
                Ok(Err(e)) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("memory.sync: {e}")),
                        result: json!({ "verified": false }),
                    })
                }
                Err(_) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some("memory.sync panic".into()),
                        result: json!(null),
                    })
                }
            };
            let peer: serde_json::Value = match serde_json::from_str(&peer_text) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("memory.sync json: {e}")),
                        result: json!(null),
                    })
                }
            };
            let inbound: Vec<String> = peer
                .get("records_b64")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let import = match state.hermes.memory.telepathy_import(&inbound) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(e.to_string()),
                        result: json!({ "peer": peer }),
                    })
                }
            };
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "telepathy": true,
                    "addr": addr,
                    "sent": local_export.len(),
                    "received": inbound.len(),
                    "import": import,
                    "peer_import": peer.get("import").cloned().unwrap_or(json!(null)),
                    "api_version": nos_memory::MEMORY_API_VERSION,
                }),
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
                    error: Some(nos_core::trim_diagnostic(&e.to_string(), true)),
                    result: json!(null),
                },
            }
        }

        _ => return None,
    })
}
