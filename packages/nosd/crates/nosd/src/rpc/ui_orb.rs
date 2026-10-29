//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "orb.get" => Response {
            id: req.id,
            ok: true,
            error: None,
            result: serde_json::to_value(state.hermes.orb_get()).unwrap_or(json!(null)),
        },
        "orb.set" => {
            let snap = state.hermes.orb_patch(&req.params);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(snap).unwrap_or(json!(null)),
            }
        }
        "orb.sense" => {
            let sense: nos_core::OrbSense =
                serde_json::from_value(req.params.clone()).unwrap_or_default();
            let snap = state.hermes.orb_sense(sense);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(snap).unwrap_or(json!(null)),
            }
        }
        "ui.list" => Response {
            id: req.id,
            ok: true,
            error: None,
            result: serde_json::to_value(state.hermes.ui_list()).unwrap_or(json!([])),
        },
        // ADR-0058 thin: CardSpec JSON → generative window (HITL card).
        "ui.card" => {
            let title = req
                .params
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("HITL")
                .to_string();
            let mut kv = Vec::new();
            if let Some(arr) = req.params.get("kv").and_then(|v| v.as_array()) {
                for item in arr {
                    if let (Some(k), Some(v)) = (
                        item.get("key").and_then(|x| x.as_str()),
                        item.get("value").and_then(|x| x.as_str()),
                    ) {
                        kv.push(nos_hermes::CardKv {
                            key: k.to_string(),
                            value: v.to_string(),
                        });
                    }
                }
            }
            let spec = nos_hermes::CardSpec { title, kv };
            match state.hermes.open_generative_card(
                gate,
                state.ipc_generation(),
                &spec,
                nos_core::UiSource::Hermes,
            ) {
                Ok(w) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "window": w,
                        "card": spec,
                        "adr": "0058",
                        "honesty": "thin CardSpec; Slint rich residual",
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
        "ui.open" => {
            let kind_s = req
                .params
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let kind = match kind_s {
                "hitl_wifi" | "wifi" => nos_core::WindowKind::HitlWifi,
                "hitl_confirm" | "confirm" => nos_core::WindowKind::HitlConfirm,
                "hitl_text" => nos_core::WindowKind::HitlText,
                "status" => nos_core::WindowKind::Status,
                "custom" => nos_core::WindowKind::Custom,
                _ => nos_core::WindowKind::Info,
            };
            let source = match req.params.get("source").and_then(|v| v.as_str()) {
                Some("cortex") => nos_core::UiSource::Cortex,
                Some("jarbas") => nos_core::UiSource::Jarbas,
                _ => nos_core::UiSource::Hermes,
            };
            let title = req
                .params
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let body = req
                .params
                .get("body")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let opened = if matches!(kind, nos_core::WindowKind::HitlWifi) {
                let ssids = wifi_nm::scan(16)
                    .map(|aps| aps.into_iter().map(|a| a.ssid).collect::<Vec<_>>())
                    .unwrap_or_default();
                state.hermes.ui_open_wifi(gate, source, ssids)
            } else {
                state.hermes.ui_open(gate, kind, source, title, body)
            };
            match opened {
                Ok(mut w) => {
                    // S-04: o opener pode fixar o tipo de grant da janela.
                    if let Some(grant_kind) = req
                        .params
                        .get("grant_kind")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        w.grant_kind = grant_kind.to_string();
                        let _ = state.hermes.windows.set_grant_kind(w.id, grant_kind);
                    }
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: serde_json::to_value(&w).unwrap_or(json!(null)),
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
        "ui.close" => {
            let id = req.params.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let ok = state.hermes.ui_close(id);
            Response {
                id: req.id,
                ok,
                error: if ok {
                    None
                } else {
                    Some("window not found".into())
                },
                result: json!({ "id": id, "closed": ok }),
            }
        }
        "ui.action" => {
            let wid = req
                .params
                .get("window_id")
                .or_else(|| req.params.get("id"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let action = match req.params.get("action").and_then(|v| v.as_str()) {
                Some("cancel") => nos_core::WindowActionKind::Cancel,
                Some("dismiss") => nos_core::WindowActionKind::Dismiss,
                _ => nos_core::WindowActionKind::Submit,
            };
            let values = req
                .params
                .get("values")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            let act = nos_core::WindowAction {
                window_id: wid,
                action,
                values: values.clone(),
            };
            // Detectar wifi antes do broker fechar a janela
            let wifi_submit = action == nos_core::WindowActionKind::Submit
                && state
                    .hermes
                    .ui_list()
                    .iter()
                    .any(|w| w.id == wid && w.kind == nos_core::WindowKind::HitlWifi);
            match state.hermes.ui_action(gate, act) {
                Ok(mut v) => {
                    // S-04: Submit de janela HITL emite grant single-use.
                    if action == nos_core::WindowActionKind::Submit {
                        let kind = v
                            .get("grant_kind")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !kind.is_empty() {
                            let (confirm_id, expires_ms) = state.hitl.issue(&kind);
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert("confirm_id".into(), json!(confirm_id));
                                obj.insert("confirm_expires_ms".into(), json!(expires_ms));
                                obj.insert("grant_kind".into(), json!(kind));
                            }
                        }
                    }
                    if wifi_submit {
                        let ssid = values
                            .get("ssid")
                            .or_else(|| values.get("text"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("");
                        let password = values
                            .get("password")
                            .and_then(|x| x.as_str())
                            .unwrap_or("");
                        match wifi_nm::connect(ssid, password) {
                            Ok(()) => {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("applied".into(), json!(true));
                                    obj.insert("reason".into(), json!("nmcli_ok"));
                                    obj.insert("ssid".into(), json!(ssid));
                                }
                            }
                            Err(e) => {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("applied".into(), json!(false));
                                    obj.insert("reason".into(), json!(e.to_string()));
                                    obj.insert("ssid".into(), json!(ssid));
                                }
                            }
                        }
                    }
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: v,
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

        // V4 (plano vampiro): fila HITL com tier — o nonce nunca sai do servidor.
        "ui.pending" => {
            let windows = state.hermes.ui_list();
            let pending: Vec<serde_json::Value> = windows
                .iter()
                .filter(|w| !w.grant_kind.is_empty())
                .map(|w| {
                    json!({
                        "window_id": w.id,
                        "kind": kind_name(w.kind),
                        "title": w.title,
                        "grant_kind": w.grant_kind,
                        "tier": nos_core::tier_for_grant_kind(&w.grant_kind).as_str(),
                        "escalate": w.escalate,
                    })
                })
                .collect();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "count": pending.len(),
                    "windows": pending,
                    "grants": state.hitl.summary(),
                    "ttl_semantics": "grant TTL server-authored (single-use); nonce nunca exposto",
                    "permission_memory": "not implemented (regra 2: HITL por pedido)",
                }),
            }
        }

        _ => return None,
    })
}

fn kind_name(k: nos_core::WindowKind) -> &'static str {
    match k {
        nos_core::WindowKind::Info => "info",
        nos_core::WindowKind::Status => "status",
        nos_core::WindowKind::HitlWifi => "hitl_wifi",
        nos_core::WindowKind::HitlConfirm => "hitl_confirm",
        nos_core::WindowKind::HitlText => "hitl_text",
        nos_core::WindowKind::Custom => "custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_kinds_are_stable_strings() {
        assert_eq!(kind_name(nos_core::WindowKind::HitlWifi), "hitl_wifi");
        assert_eq!(kind_name(nos_core::WindowKind::Info), "info");
        assert_eq!(kind_name(nos_core::WindowKind::Custom), "custom");
    }
}
