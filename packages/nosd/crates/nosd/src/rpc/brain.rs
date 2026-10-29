//! Brain self-upgrade RPC (ADR-0118 addenda `0118-qwen-embedded-brain`).
//!
//! `brain.plan` (Cap NONE / Auto)  = candidata + custo/ganho, sem side-effect.
//! `brain.accept`  (Escalate)      = exige grant HITL kind `brain.upgrade`
//! (server-authored via `ui.action`;
//! o dispatch global já o consumiu antes de chegar aqui);
//! executa: fetch → pin sha256 → hot_load → hub register → OPAR.
use super::*;
use nos_core::CapGate;
use nos_cortex::CapacityProber;
use nos_ipc::{Request, Response};
use serde_json::json;

/// Brain en operation agora (label/bytes/engine) — honesto: sem hub registrado
/// (boot default do spawn managed) devolve "boot default".
fn current_brain(state: &AppState) -> (String, Option<u64>, String) {
    let engine = state.hermes.hotswap.engine_name().to_string();
    for (slot, st) in state.hermes.hub.list() {
        if st.loaded {
            return (slot.name().to_string(), Some(st.bytes_len), engine);
        }
    }
    ("boot default".to_string(), None, engine)
}

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "brain.plan" => {
            // Observe puro: sem grant, sem side-effect (plan é grátis).
            let cap = CapacityProber::probe();
            let (label, bytes, engine) = current_brain(state);
            let plan =
                nos_hermes::brain_upgrade::plan(&cap, &label, bytes, &engine, state.nix_ring_armed);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(&plan).unwrap_or(json!(null)),
            }
        }
        "brain.accept" => {
            // HITL: dispatch global (Escalate) já verificou + consumiu o
            // grant kind `brain.upgrade`; o handler só Act.
            let tier = req.params.get("tier").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            if tier == 0 {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("params.tier required (2|3|4)".into()),
                    result: json!(null),
                });
            }
            let cap = CapacityProber::probe();
            let models = state.data_dir.join("models");
            match nos_hermes::brain_upgrade::accept(tier, &cap, state.nix_ring_armed, &models) {
                Ok(out) => {
                    // Registo no hub + hot_load (rollback = load do anterior).
                    let slot = nos_cortex::slot_from_path(&out.path);
                    state
                        .hermes
                        .hub
                        .register_path(slot, out.path.clone(), out.bytes);
                    let load = state.hermes.hotswap.hot_load_llama(out.path.clone());
                    let load_status = match load {
                        Ok(()) => "loaded",
                        Err(e) => {
                            opar_after_ok(
                                state,
                                "brain.accept",
                                false,
                                &format!("tier={} name={}", out.tier, out.name),
                                "brain_upgrade",
                                &format!("path={} load_err={e}", out.path.display()),
                                "fetched_load_failed",
                            );
                            warn!(target: "nosd", "brain.accept: fetch ok mas load falhou ({e}) — GGUF fica em disco");
                            return Some(Response {
                                id: req.id,
                                ok: false,
                                error: Some(format!("fetch ok, hot_load falhou: {e}")),
                                result: json!({
                                    "tier": out.tier,
                                    "path": out.path.display().to_string(),
                                    "bytes": out.bytes,
                                    "sha256": out.sha256,
                                    "load": "failed",
                                }),
                            });
                        }
                    };
                    opar_after_ok(
                        state,
                        "brain.accept",
                        true,
                        &format!("tier={} name={}", out.tier, out.name),
                        "brain_upgrade",
                        &format!(
                            "path={} bytes={} sha={}",
                            out.path.display(),
                            out.bytes,
                            &out.sha256[..8.min(out.sha256.len())]
                        ),
                        load_status,
                    );
                    info!(
                        target: "nosd",
                        "brain.accept: tier {} {} activado ({}), sha pin verificado",
                        out.tier,
                        out.name,
                        out.path.display()
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "tier": out.tier,
                            "name": out.name,
                            "path": out.path.display().to_string(),
                            "bytes": out.bytes,
                            "sha256": out.sha256,
                            "engine": state.hermes.hotswap.engine_name(),
                            "load": load_status,
                            "rollback": "nosctl load <path anterior>",
                        }),
                    }
                }
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "load": "not_started" }),
                },
            }
        }
        _ => return None,
    })
}
