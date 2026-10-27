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
        _ => return None,
    })
}
