//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    // S-06: nenhum caminho root (apply/confirm/build-vm/rollback) sem opt-in.
    if matches!(
        req.method.as_str(),
        "nix.apply" | "nix.confirm" | "nix.build_vm" | "nix.rollback"
    ) && !state.nix_ring_armed
    {
        return Some(Response {
            id: req.id,
            ok: false,
            error: Some(
                "NIX_RING_DISABLED (S-06): set NOSD_NIX_RING=1 only after HITL nonce + VM test"
                    .into(),
            ),
            result: json!({ "ring": "disarmed", "need": "NOSD_NIX_RING=1" }),
        });
    }

    Some(match req.method.as_str() {
        "nix.propose" => {
            let text = req
                .params
                .get("nix")
                .or_else(|| req.params.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match nix_managed::propose_staging(&state.data_dir, text) {
                Ok(r) => {
                    if r.syntax_ok {
                        let _ = state.hermes.record_opar(
                            "nix",
                            &format!("bytes={}", r.bytes),
                            "staging",
                            &format!("path={}", r.staging_path),
                            "syntax_ok",
                        );
                    }
                    Response {
                        id: req.id,
                        ok: r.syntax_ok,
                        error: if r.syntax_ok {
                            None
                        } else {
                            Some("nix syntax check failed (staging written)".into())
                        },
                        result: serde_json::to_value(&r).unwrap_or(json!(null)),
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
        "nix.apply" | "nix.confirm" => {
            // S-04: apply root exige grant server-authored, não booleano do cliente.
            if !super::hitl_grant_ok(state, gate, &req, "nix.apply") {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("HITL required: confirm_id from ui.action (kind=nix.apply)".into()),
                    result: json!({ "need": "confirm_id", "kind": "nix.apply" }),
                });
            }
            // Anel Cortex: arma apply root (HITL confirm = mesmo path, policy já gated)
            let method = req.method.as_str();
            match if method == "nix.confirm" {
                nix_managed::request_confirm(&state.data_dir)
            } else {
                nix_managed::request_apply(&state.data_dir)
            } {
                Ok(r) => {
                    opar_after_ok(
                        state,
                        method,
                        true,
                        "staging_ready",
                        method,
                        &format!("req={}", r.request_path),
                        "ok",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: serde_json::to_value(&r).unwrap_or(json!(null)),
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
        "nix.build_vm" => {
            if !super::hitl_grant_ok(state, gate, &req, "nix.build_vm") {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some(
                        "HITL required: confirm_id from ui.action (kind=nix.build_vm)".into(),
                    ),
                    result: json!({ "need": "confirm_id", "kind": "nix.build_vm" }),
                });
            }
            match nix_managed::request_build_vm(&state.data_dir) {
                Ok(r) => {
                    opar_after_ok(
                        state,
                        "nix.build_vm",
                        true,
                        "staging_ready",
                        "build_vm",
                        &format!("req={}", r.request_path),
                        "ok",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: serde_json::to_value(&r).unwrap_or(json!(null)),
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
        "systemctl.scoped" => {
            let unit = req
                .params
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let verb = req
                .params
                .get("verb")
                .and_then(|v| v.as_str())
                .unwrap_or("status");
            let hitl = super::hitl_grant_ok(state, gate, &req, "systemctl");
            match actuators::systemctl_scoped(gate, state.ipc_generation(), unit, verb, hitl) {
                Ok(intent) => {
                    let ran = if intent.would_run {
                        actuators::systemctl_run_stub(&intent).is_ok()
                    } else {
                        false
                    };
                    opar_after_ok(
                        state,
                        "systemctl.scoped",
                        true,
                        unit,
                        verb,
                        &format!("would_run={} ran={}", intent.would_run, ran),
                        "ok",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "unit": intent.unit,
                            "verb": intent.verb,
                            "would_run": intent.would_run,
                            "ran": ran,
                        }),
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
        "audio.volume" => {
            let pct = req
                .params
                .get("percent")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as u32;
            let hitl = req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) == Some(true);
            match actuators::pipewire_volume_plan(gate, state.ipc_generation(), pct, hitl) {
                Ok(plan) => {
                    let applied = if plan.would_run {
                        actuators::pipewire_volume_apply(&plan).is_ok()
                    } else {
                        false
                    };
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "percent": plan.percent,
                            "would_run": plan.would_run,
                            "applied": applied,
                            "note": plan.note,
                        }),
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
        "gpu.plan" => {
            let cuda = req
                .params
                .get("enable_cuda")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let drm = req.params.get("drm").and_then(|v| v.as_str()).unwrap_or("");
            match actuators::gpu_flake_plan(cuda, drm) {
                Ok(p) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "enable_cuda": p.enable_cuda,
                        "drm_hint": p.drm_hint,
                        "note": p.note,
                        "deny_power_limit": actuators::deny_gpu_power_limit_reason(),
                    }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
        "device.recipe" => {
            let name = req
                .params
                .get("name")
                .or_else(|| req.params.get("recipe"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match actuators::device_recipe_to_nix(name) {
                Ok(r) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "recipe": r.recipe_name,
                        "nix": r.nix_fragment,
                        "hitl_required": r.hitl_required,
                        "next": "nix.propose with nix=fragment",
                    }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
        "self.heal.evaluate" => {
            let recipe = nos_hermes::SelfHealRecipe {
                id: req
                    .params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("recipe")
                    .into(),
                action: req
                    .params
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("nix_fragment")
                    .into(),
                target: req
                    .params
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
            };
            let hitl = super::hitl_grant_ok(state, gate, &req, "self.heal");
            if !hitl {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("HITL required: confirm_id from ui.action (kind=self.heal)".into()),
                    result: json!({ "need": "confirm_id", "kind": "self.heal" }),
                });
            }
            let gate = CapGate::new(
                CapBits::NIX_WRITE.union(CapBits::HITL),
                state.ipc_generation(),
            );
            let d = nos_hermes::evaluate_self_heal(&gate, &recipe, hitl);
            opar_after_ok(
                state,
                "self.heal.evaluate",
                d.allow,
                &recipe.id,
                &recipe.action,
                &recipe.target,
                if d.allow { "allow" } else { "deny" },
            );
            Response {
                id: req.id,
                ok: d.allow,
                error: if d.allow {
                    None
                } else {
                    Some(d.reason.clone())
                },
                result: serde_json::to_value(&d).unwrap_or(json!(null)),
            }
        }
        "nix.rollback" => match nix_managed::request_rollback(&state.data_dir) {
            Ok(r) => {
                opar_after_ok(
                    state,
                    "nix.rollback",
                    true,
                    "generation_hint",
                    "request_rollback",
                    &format!("req={}", r.request_path),
                    "armed",
                );
                Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: serde_json::to_value(&r).unwrap_or(json!(null)),
                }
            }
            Err(e) => Response {
                id: req.id,
                ok: false,
                error: Some(e.to_string()),
                result: json!(null),
            },
        },
        "wifi.scan" => match wifi_nm::scan(24) {
            Ok(aps) => Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({ "aps": aps }),
            },
            Err(e) => Response {
                id: req.id,
                ok: false,
                error: Some(e.to_string()),
                result: json!({ "aps": [] }),
            },
        },
        "wifi.connect" => {
            if !super::hitl_grant_ok(state, gate, &req, "wifi.connect") {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some(
                        "HITL required: confirm_id from ui.action (kind=wifi.connect)".into(),
                    ),
                    result: json!({ "need": "confirm_id", "kind": "wifi.connect" }),
                });
            }
            let ssid = req
                .params
                .get("ssid")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let password = req
                .params
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match wifi_nm::connect(ssid, password) {
                Ok(()) => {
                    let _ = state.hermes.record_opar(
                        "wifi",
                        &format!("ssid={ssid}"),
                        "nmcli_connect",
                        "applied",
                        "ok",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({ "applied": true, "ssid": ssid }),
                    }
                }
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "applied": false, "ssid": ssid }),
                },
            }
        }

        _ => return None,
    })
}
