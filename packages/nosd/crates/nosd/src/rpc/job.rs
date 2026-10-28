//! M7 — `job.submit`: encaminha um job para um peer (link TCP autenticado).

use super::*;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "job.submit" => {
            let addr = req
                .params
                .get("addr")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let deadline = req
                .params
                .get("deadline_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(nos_mesh::job::DEFAULT_DEADLINE_MS);
            let budget = req
                .params
                .get("budget_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(256) as u32;

            if addr.is_empty() || prompt.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("job.submit requer addr e prompt".into()),
                    result: json!(null),
                });
            }
            let Some(identity) = state.mesh.identity() else {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("sem identidade de no (NOSD_MESH_REQUIRE_SIG/KEY)".into()),
                    result: json!({ "need": "identity" }),
                });
            };
            let pinned = state.mesh.pinned();
            let addr = addr.to_string();
            let prompt = prompt.to_string();
            // `dispatch` é sync: corre o link numa thread própria (job é raro e
            // tem deadline; não bloqueia o runtime tokio do daemon).
            let join = std::thread::spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => return Err(format!("rt: {e}")),
                };
                rt.block_on(nos_mesh::job::submit(
                    &addr, &identity, &pinned, &prompt, deadline, budget,
                ))
            });
            match join.join() {
                Ok(Ok(r)) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "job_id": base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            r.job_id
                        ),
                        "worker": r.worker,
                        "ok": r.ok,
                        "text": r.text,
                        "error": r.error,
                        "elapsed_ms": r.elapsed_ms,
                        "verified": true,
                    }),
                },
                Ok(Err(e)) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(format!("job: {e}")),
                    result: json!({ "verified": false }),
                },
                Err(_) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("job thread panic".into()),
                    result: json!(null),
                },
            }
        }
        "crdt.put" => {
            let key = req.params.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = req
                .params
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if key.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("crdt.put requer key".into()),
                    result: json!(null),
                });
            }
            let tick = req.params.get("tick").and_then(|v| v.as_u64()).unwrap_or(0);
            state.mesh.crdt_put(state.mesh.local_id(), key, value, tick);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({ "key": key, "entries": state.mesh.crdt_len() }),
            }
        }
        "crdt.get" => {
            let key = req.params.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let e = state.mesh.crdt_get(key);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "key": key,
                    "value": e.as_ref().map(|x| x.value.clone()),
                    "present": e.is_some(),
                    "entries": state.mesh.crdt_len(),
                }),
            }
        }
        "crdt.sync" => {
            let addr = req
                .params
                .get("addr")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if addr.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("crdt.sync requer addr".into()),
                    result: json!(null),
                });
            }
            let Some(identity) = state.mesh.identity() else {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("sem identidade de no".into()),
                    result: json!({ "need": "identity" }),
                });
            };
            let pinned = state.mesh.pinned();
            let local = state.mesh.crdt_snapshot();
            let payload = match serde_json::to_string(&local) {
                Ok(p) => p,
                Err(e) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("snapshot: {e}")),
                        result: json!(null),
                    })
                }
            };
            let sent = local.entries.len();
            let addr = addr.to_string();
            let join = std::thread::spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => return Err(format!("rt: {e}")),
                };
                rt.block_on(nos_mesh::job::submit_crdt(
                    &addr, &identity, &pinned, &payload, 30_000,
                ))
            });
            match join.join() {
                Ok(Ok(r)) => {
                    let applied = if r.ok {
                        match serde_json::from_str::<nos_mesh::CrdtStore>(&r.text) {
                            Ok(remote) => {
                                state.mesh.crdt_merge(&remote);
                                remote.entries.len()
                            }
                            Err(_) => 0,
                        }
                    } else {
                        0
                    };
                    Response {
                        id: req.id,
                        ok: r.ok,
                        error: r.error,
                        result: json!({
                            "worker": r.worker,
                            "verified": true,
                            "sent": sent,
                            "received": applied,
                            "entries": state.mesh.crdt_len(),
                        }),
                    }
                }
                Ok(Err(e)) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(format!("crdt sync: {e}")),
                    result: json!({ "verified": false }),
                },
                Err(_) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("crdt sync panic".into()),
                    result: json!(null),
                },
            }
        }
        _ => return None,
    })
}
