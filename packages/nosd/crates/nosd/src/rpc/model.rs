//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "model.list" => {
            let list: Vec<_> = state
                .hermes
                .hub
                .list()
                .into_iter()
                .map(|(slot, st)| {
                    json!({
                        "slot": slot.name(),
                        "loaded": st.loaded,
                        "path": st.path.map(|p| p.display().to_string()),
                        "bytes": st.bytes_len,
                    })
                })
                .collect();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!(list),
            }
        }
        "model.suggest" => {
            let cap = nos_cortex::CapacityProber::probe();
            let s = nos_cortex::suggest_for_capacity(&cap);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(&s).unwrap_or(json!(null)),
            }
        }
        "model.plan" => {
            // Capacity + HF search ? recomendação (sem download)
            let cap = nos_cortex::CapacityProber::probe();
            let plan = nos_cortex::plan_models_for_capacity(&cap);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(&plan).unwrap_or(json!(null)),
            }
        }
        "model.search" => {
            let q = req
                .params
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("gguf");
            let lim = req
                .params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(8) as u32;
            match nos_cortex::search_hf_gguf(q, lim) {
                Ok(hits) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "query": q, "hits": hits }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
        "blob.deliberate" | "model.fetch" | "firmware.fetch" => {
            // ADR-0113 / remigração: blobs externos só runtime, deliberados Cortex?Hermes?JARBAS.
            // ISO NÃO embute modelos/firmware (iso_must_not_ship_blobs).
            let kind = match req.method.as_str() {
                "firmware.fetch" => BlobKind::Firmware,
                _ => match req.params.get("kind").and_then(|v| v.as_str()) {
                    Some("firmware") => BlobKind::Firmware,
                    Some("whisper") => BlobKind::VoiceWhisper,
                    Some("piper") => BlobKind::VoicePiper,
                    Some("bitnet") => BlobKind::ModelBitnet,
                    Some("wasm") => BlobKind::WasmSkill,
                    _ => BlobKind::ModelGguf,
                },
            };
            let source = req
                .params
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    let repo = req.params.get("repo").and_then(|v| v.as_str())?;
                    let file = req.params.get("file").and_then(|v| v.as_str())?;
                    Some(format!("hf:{repo}/{file}"))
                })
                .unwrap_or_default();
            if source.is_empty() && req.method != "blob.deliberate" {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("need params.url or params.repo+file".into()),
                    result: json!({ "iso_policy": iso_must_not_ship_blobs() }),
                });
            }
            let est_mb = req
                .params
                .get("estimated_mb")
                .and_then(|v| v.as_u64())
                .unwrap_or(if matches!(kind, BlobKind::Firmware) {
                    8
                } else {
                    500
                });
            let need = BlobNeed {
                kind,
                goal: req
                    .params
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("runtime blob")
                    .into(),
                source: source.clone(),
                estimated_mb: est_mb,
            };
            let cap = CapacityProber::probe();
            let hitl = req
                .params
                .get("hitl_confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let scorer_absent = matches!(s1_backend_from_env(), S1Backend::Absent);
            let plan = deliberate_blob_fetch(&need, &cap, hitl, scorer_absent);
            let _ = remember_deliberation(&state.hermes.memory, &plan);
            if req.method == "blob.deliberate" || !plan.allow_fetch {
                let mut window_id = None;
                if plan.hitl_required && !hitl {
                    let id = state.hermes.windows.alloc_id();
                    let body = format!(
                        "{}\nplan={}\nfit={:?}\n{}",
                        plan.reason, plan.cortex_plan, plan.fit, iso_must_not_ship_blobs()
                    );
                    let spec = WindowSpec::hitl_confirm(
                        id,
                        format!("Blob fetch {:?}", need.kind),
                        body,
                        UiSource::Hermes,
                    );
                    if let Ok(opened) = state.hermes.windows.open(&gate, state.cap_gen, spec) {
                        window_id = Some(opened.id);
                    } else {
                        window_id = Some(id);
                    }
                }
                return Some(Response {
                    id: req.id,
                    ok: plan.allow_fetch && req.method != "blob.deliberate",
                    error: if plan.allow_fetch {
                        None
                    } else {
                        Some(plan.reason.clone())
                    },
                    result: json!({
                        "deliberation": plan,
                        "need_hitl": plan.hitl_required && !hitl,
                        "window_id": window_id,
                        "iso_policy": iso_must_not_ship_blobs(),
                        "runtime_only": true,
                    }),
                });
            }
            // Act: download só após deliberação + HITL se exigido.
            if matches!(kind, BlobKind::Firmware) {
                let fw_dir = state.data_dir.join("firmware");
                let name = req
                    .params
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .or_else(|| source.rsplit('/').next())
                    .unwrap_or("fw.bin");
                let url = req.params.get("url").and_then(|v| v.as_str()).unwrap_or("");
                return match nos_cortex::fetch_to_dir(url, &fw_dir, name) {
                    Ok(path) => {
                        let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        opar_after_ok(
                            state,
                            "firmware.fetch",
                            true,
                            &format!("url={url}"),
                            "deliberated_fetch",
                            &format!("path={} bytes={len}", path.display()),
                            "ok",
                        );
                        Some(Response {
                            id: req.id,
                            ok: true,
                            error: None,
                            result: json!({
                                "path": path.display().to_string(),
                                "bytes": len,
                                "kind": "firmware",
                                "deliberation": plan,
                                "runtime_only": true,
                            }),
                        })
                    }
                    Err(e) => Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(e.to_string()),
                        result: json!({ "deliberation": plan }),
                    }),
                };
            }
            let models_dir = state.data_dir.join("models");
            let dest = match (
                req.params.get("url").and_then(|v| v.as_str()),
                req.params.get("repo").and_then(|v| v.as_str()),
                req.params.get("file").and_then(|v| v.as_str()),
            ) {
                (Some(url), _, _) if !url.is_empty() => {
                    let name = req
                        .params
                        .get("filename")
                        .and_then(|v| v.as_str())
                        .or_else(|| url.rsplit('/').next())
                        .unwrap_or("model.gguf");
                    nos_cortex::fetch_to_dir(url, &models_dir, name)
                }
                (_, Some(repo), Some(file)) if !repo.is_empty() && !file.is_empty() => {
                    nos_cortex::fetch_hf_gguf(repo, file, &models_dir)
                }
                _ => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some("need params.url or params.repo+file".into()),
                        result: json!({ "deliberation": plan }),
                    });
                }
            };
            match dest {
                Ok(path) => {
                    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let slot = nos_cortex::slot_from_path(&path);
                    state.hermes.hub.register_path(slot, path.clone(), len);
                    opar_after_ok(
                        state,
                        "model.fetch",
                        true,
                        &format!("source={source}"),
                        "deliberated_fetch",
                        &format!("slot={} bytes={len}", slot.name()),
                        "registered",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "path": path.display().to_string(),
                            "bytes": len,
                            "slot": slot.name(),
                            "deliberation": plan,
                            "runtime_only": true,
                        }),
                    }
                }
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "deliberation": plan }),
                },
            }
        }
        "model.load" => {
            let path = req
                .params
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if path.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("params.path required".into()),
                    result: json!(null),
                });
            }
            let p = PathBuf::from(path);
            match state.hermes.hotswap.hot_load_llama(p.clone()) {
                Ok(()) => {
                    let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    let slot = nos_cortex::slot_from_path(&p);
                    state.hermes.hub.register_path(slot, p.clone(), len);
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "engine": state.hermes.hotswap.engine_name(),
                            "path": p.display().to_string(),
                            "slot": slot.name(),
                        }),
                    }
                }
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({
                        "engine": state.hermes.hotswap.engine_name(),
                    }),
                },
            }
        }
        "model.unload" => {
            state.hermes.hotswap.unload();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "engine": state.hermes.hotswap.engine_name(),
                }),
            }
        }

        _ => return None,
    })
}
