//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "factory.seek" | "store.search" => {
            let goal = req
                .params
                .get("goal")
                .or_else(|| req.params.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("skill");
            let need = nos_factory::SkillNeed {
                goal: goal.into(),
                capabilities: vec![],
                frugal_budget: true,
            };
            let report = state.hermes.seek_before_spawn(&need);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(&report).unwrap_or(json!({
                    "synthesize_allowed": report.synthesize_allowed,
                })),
            }
        }
        "store.install" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let hash = req
                .params
                .get("content_hash")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let body_b64 = req.params.get("body_b64").and_then(|v| v.as_str());
            let body_text = req.params.get("body").and_then(|v| v.as_str());
            let body: Vec<u8> = if let Some(b64) = body_b64 {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
                {
                    Ok(b) => b,
                    Err(e) => {
                        return Some(Response {
                            id: req.id,
                            ok: false,
                            error: Some(format!("body_b64: {e}")),
                            result: json!(null),
                        });
                    }
                }
            } else {
                body_text.unwrap_or("").as_bytes().to_vec()
            };
            let sig = req
                .params
                .get("signature")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let pk = req
                .params
                .get("public_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let meta = ImportMeta {
                name: name.into(),
                content_hash: hash,
                signature: sig,
                public_key: pk,
                provenance: ImportProvenance::ImportedPublic,
                sandbox_status: SandboxStatus::Passed,
            };
            let mut store = state.package_store.lock();
            match store.install_imported(meta, &body) {
                Ok(pkg) => {
                    let installed = json!({
                        "name": pkg.meta.name.clone(),
                        "body_len": pkg.body_len,
                        "sandbox": format!("{:?}", pkg.meta.sandbox_status),
                    });
                    let body_len = pkg.body_len;
                    drop(store);
                    opar_after_ok(
                        state,
                        "store.install",
                        true,
                        &format!("name={name}"),
                        "package_store",
                        &format!("body_len={body_len}"),
                        "installed",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: installed,
                    }
                }
                Err(StoreError::Unsigned) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("unsigned - content_hash required".into()),
                    result: json!(null),
                },
                Err(StoreError::EmptyName) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("empty name".into()),
                    result: json!(null),
                },
                Err(StoreError::SandboxNotPassed) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("sandbox_status must be passed".into()),
                    result: json!(null),
                },
                Err(StoreError::HashMismatch) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("content_hash mismatch (fnv1a64)".into()),
                    result: json!(null),
                },
                Err(StoreError::InvalidHash) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("invalid content_hash hex".into()),
                    result: json!(null),
                },
                Err(StoreError::InvalidSignatureHex) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("invalid ed25519 signature/key hex".into()),
                    result: json!(null),
                },
                Err(StoreError::SignatureBad) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("ed25519 verify failed".into()),
                    result: json!(null),
                },
                Err(StoreError::MissingPublicKey) => Response {
                    id: req.id,
                    ok: false,
                    error: Some("signature without public_key".into()),
                    result: json!(null),
                },
            }
        }
        "factory.synthesize" => {
            // NX.0 / F.3: Seek-before-generate embutido em spawn_wasm_agent.
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("synth");
            match state.hermes.spawn_wasm_agent(&gate, name, &[]) {
                Ok(v) => {
                    let synth = v.get("synthesized").and_then(|x| x.as_bool()).unwrap_or(true);
                    opar_after_ok(
                        state,
                        "factory.synthesize",
                        true,
                        &format!("name={name}"),
                        v.get("seek").and_then(|s| s.as_str()).unwrap_or("seek"),
                        if synth { "path_a_frugal" } else { "seek_hit" },
                        "ok",
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({ "synthesized": synth, "agent": v }),
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
        "store.index" => {
            let idx = state.hermes.federated_index.lock();
            let entries: Vec<_> = idx
                .entries()
                .map(|e| {
                    json!({
                        "name": e.name,
                        "url": e.url,
                        "source": format!("{:?}", e.source),
                        "score": e.score,
                        "sandbox": format!("{:?}", e.sandbox_status),
                        "escalate_license": e.escalate_license,
                    })
                })
                .collect();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({ "entries": entries, "len": entries.len() }),
            }
        }
        "agent.spawn" | "factory.promote" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("agent");
            let goal = req
                .params
                .get("goal")
                .and_then(|v| v.as_str())
                .unwrap_or(name);
            // F.3 Seek-Before-Generate when no wasm payload provided.
            let has_wasm = req
                .params
                .get("wasm_b64")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !has_wasm {
                let need = nos_factory::SkillNeed {
                    goal: goal.into(),
                    capabilities: vec![],
                    frugal_budget: true,
                };
                let report = state.hermes.seek_before_spawn(&need);
                if report.hit.is_some() {
                    return Some(Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "seek_hit": true,
                            "report": serde_json::to_value(&report).unwrap_or(json!(null)),
                            "note": "import candidate - use store.install; synthesize skipped",
                        }),
                    });
                }
                if !report.synthesize_allowed {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(
                            report
                                .synthesize_reason
                                .clone()
                                .unwrap_or_else(|| "Seek incomplete - NetAbsent or pending".into()),
                        ),
                        result: serde_json::to_value(&report).unwrap_or(json!(null)),
                    });
                }
            }
            // omit wasm_b64 ? Frugal identity; else decode standard base64 ? path A validate
            let wasm_bytes: Vec<u8> = match req.params.get("wasm_b64").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    s.trim(),
                ) {
                    Ok(b) => b,
                    Err(e) => {
                        return Some(Response {
                            id: req.id,
                            ok: false,
                            error: Some(format!("wasm_b64 decode: {e}")),
                            result: json!(null),
                        });
                    }
                },
                _ => Vec::new(),
            };
            match state.hermes.spawn_wasm_agent(&gate, name, &wasm_bytes) {
                Ok(v) => {
                    let method = if req.method == "factory.promote" {
                        "factory.promote"
                    } else {
                        "agent.spawn"
                    };
                    opar_after_ok(
                        state,
                        method,
                        true,
                        &format!("name={name} goal={goal}"),
                        "seek_or_wasm",
                        "spawn_wasm_agent",
                        "ok",
                    );
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

        _ => return None,
    })
}
