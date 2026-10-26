//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "health" => {
            sync_local_fleet_health(state);
            let machines = state.mesh.fleet_machines();
            let fleet_verdict = state.mesh.fleet_verdict();
            if state.mesh.peer_count() > 0 || machines.len() > 1 {
                state.hermes.fleet_hud_from_mesh(
                    fleet_verdict,
                    state.mesh.peer_count(),
                    machines.len() as u32,
                );
            }
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "status": "ok",
                    "mesh": state.mesh.health_json(),
                    "mesh_peers": state.mesh.peer_count(),
                    "fleet_verdict": fleet_verdict.map(|v| v.as_str()),
                    "fleet_machines": machines.len(),
                    "hub_loaded": state.hermes.hub.loaded_count(),
                    "engine": state.hermes.hotswap.engine_name(),
                    "llama_server": nos_cortex::llama_server_posture(state.hermes.hotswap.engine_name()),
                    "s1_backend": s1_backend_label(),
                    "landlock": format!("{:?}", crate::landlock::applied_status()),
                    "s2_pending": state.hermes.s2.pending(),
                    "hermes": if state.hermes_ticks.load(std::sync::atomic::Ordering::Relaxed) > 0 {
                        "running"
                    } else {
                        "starting"
                    },
                    "hermes_ticks": state.hermes_ticks.load(std::sync::atomic::Ordering::Relaxed),
                    "orb": serde_json::to_value(state.hermes.orb_get()).unwrap_or(json!(null)),
                    "orb_lab": "http://127.0.0.1:7701/",
                }),
            }
        }
        "fleet.health" => {
            sync_local_fleet_health(state);
            let machines = state.mesh.fleet_machines();
            let verdict = state
                .mesh
                .fleet_verdict()
                .unwrap_or(HealthVerdict::Unknown);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "verdict": verdict.as_str(),
                    "worst_of": fleet_worst(&machines).as_str(),
                    "machines": machines,
                    "mesh_peers": state.mesh.peer_count(),
                    "honesty": "MCH wire partial - no auto TX on HB; CRDT NOT_DONE",
                }),
            }
        }
        "observe.snapshot" => {
            let snap = nos_observe::collect_snapshot();
            state.hermes.bus.publish(
                nos_observe::TOPIC_OBSERVE_SNAPSHOT,
                nos_observe::snapshot_bus_payload(&snap),
            );
            // NX.5: thermal/presence ? env ngl cap (não força llama; hint Continuous).
            if snap.suggest_ngl_cap < 99 {
                let cur = std::env::var("NOSD_LLAMA_NGL")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(99);
                if snap.suggest_ngl_cap < cur {
                    std::env::set_var("NOSD_LLAMA_NGL", snap.suggest_ngl_cap.to_string());
                }
            }
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(&snap).unwrap_or(json!(null)),
            }
        }
        "policy.matrix" => {
            let rows: Vec<_> = nos_core::policy_matrix_prod()
                .into_iter()
                .map(|(d, x)| {
                    json!({
                        "domain": format!("{d:?}"),
                        "disposition": format!("{x:?}"),
                    })
                })
                .collect();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({ "prod": rows, "lab_auto_switch": lab_auto_switch_from_env() }),
            }
        }
        "route.explain" => {
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let d = state.hermes.explain_route(prompt);
            let sc = nos_cortex::score_intent(prompt);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "slot": d.slot.name(),
                    "lane": match d.lane {
                        nos_cortex::SystemLane::S1Local => "s1",
                        nos_cortex::SystemLane::S2Deep => "s2",
                    },
                    "reason": d.reason,
                    "scores": {
                        "hw": sc.hw,
                        "rust": sc.rust,
                        "complex": sc.complex,
                        "greet": sc.greet,
                    },
                }),
            }
        }
        "cognitive.tick" => {
            let mut v = state.hermes.cognitive_tick();
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "mesh_peers".into(),
                    json!(state.mesh.peer_count()),
                );
                obj.insert(
                    "mesh_last_frag".into(),
                    json!(state.mesh.last_frag_len()),
                );
            }
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: v,
            }
        }
        "artifact.validate" => {
            let text = req
                .params
                .get("md")
                .or_else(|| req.params.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match nos_factory::validate_artifact_md(text) {
                Ok(m) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: serde_json::to_value(&m).unwrap_or(json!(null)),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }

        "score" => {
            sync_local_fleet_health(state);
            let s = build_runtime_score(state, "runtime", "ok");
            info!(target: "nosd", "{}", s.line());
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(&s).unwrap_or(json!(null)),
            }
        }

        _ => return None,
    })
}
