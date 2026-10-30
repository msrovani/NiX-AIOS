//! RPC handlers — fatia 0.5.2 (+ V0/V1/V3 do plano vampiro).
use super::*;
use nos_core::{CapGate, CostModel, ScoreWeights};
use nos_ipc::{Request, Response};
use serde_json::json;
use std::collections::BTreeMap;

/// Variáveis `NOSD_*` visíveis no `config.show` — **nunca** segredos.
///
/// Filtro por nome (fail-closed): qualquer chave que contenha TOKEN/SECRET/
/// KEY/PASS/PUBLISHER fica de fora, mesmo que comece por `NOSD_`.
fn filtered_env() -> BTreeMap<String, String> {
    filter_env_pairs(std::env::vars())
}

/// Parte pura do filtro (testável sem mexer no ambiente do processo).
fn filter_env_pairs<I>(pairs: I) -> BTreeMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    const DENY: &[&str] = &["TOKEN", "SECRET", "KEY", "PASS", "PUBLISHER"];
    let mut out = BTreeMap::new();
    for (k, v) in pairs {
        if !k.starts_with("NOSD_") {
            continue;
        }
        let upper = k.to_ascii_uppercase();
        if DENY.iter().any(|d| upper.contains(d)) {
            continue;
        }
        out.insert(k, v);
    }
    out
}

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
                    "mesh_pinned": state.mesh.pinned_peers(),
                    "mesh_node_id": state.mesh.identity_id(),
                    "fleet_verdict": fleet_verdict.map(|v| v.as_str()),
                    "fleet_machines": machines.len(),
                    "hub_loaded": state.hermes.hub.loaded_count(),
                    "memory_records": state.hermes.memory.record_count().unwrap_or(0),
                    // Prova de GPU: o binário e o que o spawn vai pedir.
                    "gpu": {
                        "nvidia_smi": nos_core::nvidia_smi_path()
                            .map(|p| p.display().to_string()),
                        "ngl": std::env::var("NOSD_LLAMA_NGL").unwrap_or_default(),
                        "cuda_visible": std::env::var("CUDA_VISIBLE_DEVICES").unwrap_or_default(),
                    },
                    "engine": state.hermes.hotswap.engine_name(),
                    "engine_s2": state.hermes.hotswap_s2.as_ref().map(|c| c.engine_name()),
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
            let verdict = state.mesh.fleet_verdict().unwrap_or(HealthVerdict::Unknown);
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
            let plan = state.hermes.plan_route(prompt, true);
            let sc = nos_cortex::score_intent(prompt);
            // ADR-0106: ScorerAbsent via helper; Decision ligada a score_intent_s1 (não hardcode).
            let s1_want = nos_core::s1_backend_from_env();
            let s1_eff = nos_core::s1_backend_effective();
            let blob = nos_core::s1_blob_present();
            let hub_agent = state
                .hermes
                .hub
                .path_of(nos_core::ModelSlot::Agent)
                .is_some();
            let decision = if nos_core::s1_scorer_absent()
                && matches!(
                    s1_want,
                    nos_core::S1Backend::AgentJev | nos_core::S1Backend::DestilledLaya
                ) {
                nos_core::Decision::<String>::abstain(
                    nos_core::AbstainReason::ScorerAbsent,
                    "s1_loo_blob_deliberate",
                )
            } else {
                let s1 = nos_core::score_intent_s1(prompt);
                match (s1.abstain, s1.value) {
                    (Some(reason), _) => nos_core::Decision::<String>::abstain(reason, s1.source),
                    (None, Some(slot)) => nos_core::Decision::decide(
                        slot.name().to_string(),
                        s1.confidence_q8,
                        s1.margin_q8,
                        s1.source,
                    ),
                    (None, None) => nos_core::Decision::decide(
                        d.slot.name().to_string(),
                        s1.confidence_q8,
                        s1.margin_q8,
                        format!("route_fallback:{}", d.reason),
                    ),
                }
            };
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
                    "trinity": {
                        "kind": "score_v0",
                        "honesty": "≠ ROUTER.BITNET / Trinity neural bare; hub 8 slots + route_intent",
                        "idea": "#535",
                        "adr": "0078",
                    },
                    "s1": {
                        "backend_wanted": format!("{s1_want:?}"),
                        "backend_effective": format!("{s1_eff:?}"),
                        "backend_label": nos_core::s1_backend_label(),
                        "blob_present": blob,
                        "hub_agent_path": hub_agent,
                        "scorer_absent": nos_core::s1_scorer_absent(),
                        "loo": if !blob { "blob_deliberate" } else { "ready" },
                    },
                    "decision": decision,
                    // V10: quem responde agora (S1 0.8B CPU / S2 4B GPU).
                    "plan": {
                        "backend": format!("{:?}", plan.backend),
                        "reason": plan.reason,
                        "downgraded": plan.downgraded,
                        "s2_available": state.hermes.hotswap_s2.is_some(),
                        "s2_pending": state.hermes.s2.pending(),
                        "power_save": nos_core::routing::power_save_from_env(),
                    },
                    "hitl_cards": {
                        "adr": "0058",
                        "kinds": [
                            "hitl_wifi", "hitl_confirm", "hitl_text",
                            "info", "custom", "status"
                        ],
                        "prompt_prefix": "card:",
                        "rpc": ["ui.open", "ui.list", "ui.card", "ui.action"],
                        "open_windows": state.hermes.ui_list().len(),
                        "honesty": "thin CardSpec→WindowBroker; Slint rich widgets residual",
                    },
                    "m3_close": {
                        "engine": state.hermes.hotswap.engine_name(),
                        "cpu_echo_documented": state
                            .hermes
                            .hotswap
                            .engine_name()
                            .contains("cpu-echo")
                            || state
                                .hermes
                                .hotswap
                                .engine_name()
                                .contains("echo"),
                        "hub_loaded": state.hermes.hub.loaded_count(),
                        "s1_loo": if !blob { "blob_deliberate" } else { "ready" },
                        "jarvis_like": "M3b cage + (GGUF|cpu-echo) + voice Ready|Absent + M3c remember",
                    },
                }),
            }
        }
        "cognitive.tick" => {
            let mut v = state.hermes.cognitive_tick();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("mesh_peers".into(), json!(state.mesh.peer_count()));
                obj.insert("mesh_last_frag".into(), json!(state.mesh.last_frag_len()));
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

        // V0 (plano vampiro): configuração efectiva num só sítio, sem segredos.
        "config.show" => {
            let (weights, weights_error) = ScoreWeights::from_env();
            let cost = CostModel::from_env();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "source": "daemon",
                    "data_dir": state.data_dir.display().to_string(),
                    "engine": state.hermes.hotswap.engine_name(),
                    "hub_loaded": state.hermes.hub.loaded_count(),
                    "s1_backend": s1_backend_label(),
                    "landlock": format!("{:?}", crate::landlock::applied_status()),
                    "hermes_tick_secs": std::env::var("NOSD_HERMES_TICK_SECS").unwrap_or_default(),
                    "mesh_bind": std::env::var("NOSD_MESH_BIND").unwrap_or_default(),
                    "nix_ring_armed": state.nix_ring_armed,
                    "lab_auto_switch": lab_auto_switch_from_env(),
                    "score_weights": weights,
                    "score_weights_error": weights_error,
                    "cost_model": cost,
                    "env": filtered_env(),
                    "secrets_filtered": true,
                }),
            }
        }

        // V1 (plano vampiro): energia medida/estimada + custo local acumulado.
        "observe.energy" => {
            let delta = state.energy.lock().sample();
            let cost = CostModel::from_env();
            // V8.2: cobertura por turno (uma rate que carrega o denominador).
            let total = state.turns_total.load(std::sync::atomic::Ordering::Relaxed);
            let measured = state
                .turns_measured
                .load(std::sync::atomic::Ordering::Relaxed);
            let coverage = nos_core::reported_rate(
                measured,
                total,
                &["rapl", "nvml", "tdp_estimate"],
                "chat_turns",
                &["s2_pending", "no_energy_source"],
            )
            .ok()
            .and_then(|r| serde_json::to_value(r).ok())
            .unwrap_or(json!(null));
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "joules": delta.joules,
                    "joules_total": delta.joules_total,
                    "watts": delta.watts,
                    "source": delta.source,
                    "scope": delta.scope,
                    "ts_ms": delta.ts_ms,
                    "coverage": coverage,
                    "cost": {
                        "local_eur_total": cost.local_eur(delta.joules_total),
                        "kwh_price_eur": cost.kwh_price_eur,
                        "ref_date": cost.ref_date,
                        "honesty": "local = medido/estimado (ver source); cloud só por turno com tokens",
                    },
                    "rapl_candidates": nos_observe::energy::RAPL_CANDIDATES,
                }),
            }
        }

        // V3 (plano vampiro): trace OPAR — metadados, nunca notas/PII.
        "cognitive.trace" => {
            let last = req
                .params
                .get("last")
                .and_then(|v| v.as_u64())
                .unwrap_or(8)
                .clamp(1, 64) as usize;
            let acts = state.hermes.opar_trace(last);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "count": acts.len(),
                    "acts": acts,
                    "redaction": "domain+lengths only (no prompt/PII)",
                }),
            }
        }

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_env_filter_drops_secrets_by_name() {
        let pairs = vec![
            ("NOSD_HERMES_TICK_SECS".to_string(), "5".to_string()),
            ("NOSD_IPC_TOKEN".to_string(), "deadbeef".to_string()),
            ("NOSD_MESH_KEY".to_string(), "/run/key".to_string()),
            ("NOSD_WIFI_PASSWORD".to_string(), "x".to_string()),
            ("NOSD_SKILL_PUBLISHERS".to_string(), "aa".to_string()),
            ("PATH".to_string(), "/usr/bin".to_string()),
        ];
        let out = filter_env_pairs(pairs);
        assert_eq!(out.get("NOSD_HERMES_TICK_SECS"), Some(&"5".to_string()));
        assert!(!out.contains_key("NOSD_IPC_TOKEN"));
        assert!(!out.contains_key("NOSD_MESH_KEY"));
        assert!(!out.contains_key("NOSD_WIFI_PASSWORD"));
        assert!(!out.contains_key("NOSD_SKILL_PUBLISHERS"));
        assert!(!out.contains_key("PATH"));
    }

    #[test]
    fn config_env_filter_is_empty_safe() {
        assert!(filter_env_pairs(Vec::<(String, String)>::new()).is_empty());
    }
}
