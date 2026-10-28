//! M7 — `job.submit`: encaminha um job para um peer (link TCP autenticado).

use super::*;
use nos_ipc::{Request, Response};
use serde_json::json;

fn err(id: u64, msg: impl Into<String>) -> Response {
    Response {
        id,
        ok: false,
        error: Some(msg.into()),
        result: json!(null),
    }
}

pub(crate) fn dispatch(state: &AppState, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "job.submit" => {
            let addr = req
                .params
                .get("addr")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
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
                .unwrap_or("")
                .to_string();
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
            // M8.1: pull (envia so o VV) e push (envia o delta que o peer nao tem).
            let vv0 = state.mesh.crdt_version();
            let pinned = state.mesh.pinned();
            let id1 = identity.clone();
            let pin1 = pinned.clone();
            let a1 = addr.clone();
            let pull = std::thread::spawn(move || -> Result<(Vec<nos_mesh::CrdtEntry>, std::collections::HashMap<u32, u64>), String> {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                rt.block_on(async move {
                    let req = serde_json::json!({ "vv": vv0, "entries": [] });
                    let r = nos_mesh::job::submit_crdt(&a1, &id1, &pin1, &req.to_string(), 30_000).await?;
                    if !r.ok {
                        return Err(r.error.unwrap_or_default());
                    }
                    let d: serde_json::Value =
                        serde_json::from_str(&r.text).map_err(|e| e.to_string())?;
                    let entries: Vec<nos_mesh::CrdtEntry> =
                        serde_json::from_value(d["entries"].clone()).map_err(|e| e.to_string())?;
                    let vv = serde_json::from_value(d["vv"].clone()).unwrap_or_default();
                    Ok((entries, vv))
                })
            });
            let (pulled, server_vv) = match pull.join() {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("crdt pull: {e}")),
                        result: json!({ "verified": false }),
                    })
                }
                Err(_) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some("crdt pull panic".into()),
                        result: json!(null),
                    })
                }
            };
            let received = pulled.len();
            state.mesh.crdt_merge_entries(&pulled);

            let push = state.mesh.crdt_delta_since(&server_vv);
            let sent = push.len();
            let id2 = identity.clone();
            let pin2 = pinned.clone();
            let a2 = addr.clone();
            let svv = server_vv.clone();
            let push_thread = std::thread::spawn(move || -> Result<usize, String> {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                rt.block_on(async move {
                    if push.is_empty() {
                        return Ok(0);
                    }
                    let req = serde_json::json!({ "vv": svv, "entries": push });
                    let r = nos_mesh::job::submit_crdt(&a2, &id2, &pin2, &req.to_string(), 30_000)
                        .await?;
                    if !r.ok {
                        return Err(r.error.unwrap_or_default());
                    }
                    let d: serde_json::Value =
                        serde_json::from_str(&r.text).map_err(|e| e.to_string())?;
                    let back: Vec<nos_mesh::CrdtEntry> =
                        serde_json::from_value(d["entries"].clone()).unwrap_or_default();
                    Ok(back.len())
                })
            });
            match push_thread.join() {
                Ok(Ok(back)) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "verified": true,
                        "received": received + back,
                        "sent": sent,
                        "entries": state.mesh.crdt_len(),
                    }),
                },
                Ok(Err(e)) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(format!("crdt push: {e}")),
                    result: json!({ "verified": true, "received": received }),
                },
                Err(_) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("crdt push panic".into()),
                    result: json!(null),
                },
            }
        }
        "skill.push" => {
            let addr = req
                .params
                .get("addr")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if addr.is_empty() || name.is_empty() {
                return Some(err(req.id, "skill.push requer addr e name"));
            }
            let Some(entry) = state.mesh.crdt_get(&format!("skill/{name}/active")) else {
                return Some(err(
                    req.id,
                    format!("skill '{name}' nao instalada localmente"),
                ));
            };
            let hash = entry.value;
            let path = crate::rpc::skills::skills_dir(state).join(format!("{hash}.wasm"));
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return Some(err(req.id, format!("modulo {hash}: {e}"))),
            };
            let Some(identity) = state.mesh.identity() else {
                return Some(err(req.id, "sem identidade de no"));
            };
            let pinned = state.mesh.pinned();
            let n = name.clone();
            let join = std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                rt.block_on(nos_mesh::job::submit_skill(
                    &addr, &identity, &pinned, &n, &bytes, 30_000,
                ))
            });
            match join.join() {
                Ok(Ok(r)) => Response {
                    id: req.id,
                    ok: r.ok,
                    error: r.error,
                    result: json!({
                        "name": name,
                        "hash": hash,
                        "worker": r.worker,
                        "verified": true,
                        "text": r.text,
                        "elapsed_ms": r.elapsed_ms,
                    }),
                },
                Ok(Err(e)) => err(req.id, format!("skill.push: {e}")),
                Err(_) => err(req.id, "skill.push panic"),
            }
        }
        _ => return None,
    })
}
