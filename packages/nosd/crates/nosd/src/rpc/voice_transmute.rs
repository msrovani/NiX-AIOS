//! RPC handlers — fatia 0.5.2.
use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "voice.status" => {
            let (models_dir, _, _) = transmute::resolve_models_dir(&state.data_dir);
            let vp = nos_voice::VoicePipeline::with_models_dir(models_dir);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(vp.status()).unwrap_or(json!(null)),
            }
        }
        "voice.stt" => {
            let (models_dir, _, _) = transmute::resolve_models_dir(&state.data_dir);
            let vp = nos_voice::VoicePipeline::with_models_dir(models_dir);
            let res = if let Some(b64) = req.params.get("wav_b64").and_then(|v| v.as_str()) {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
                {
                    Ok(bytes) => {
                        let tmp = std::env::temp_dir().join("nosd-rpc-stt.wav");
                        let _ = std::fs::write(&tmp, bytes);
                        vp.transcribe_wav_path(&tmp)
                    }
                    Err(e) => Err(nos_voice::VoiceError::Exec(format!("base64: {e}"))),
                }
            } else if let Some(p) = req.params.get("wav_path").and_then(|v| v.as_str()) {
                vp.transcribe_wav_path(std::path::Path::new(p))
            } else {
                let secs = req
                    .params
                    .get("record_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(4) as u32;
                let vad = req
                    .params
                    .get("vad")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if vad {
                    vp.record_and_transcribe_vad(secs.max(10))
                } else {
                    vp.record_and_transcribe(secs)
                }
            };
            match res {
                Ok(text) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "text": text,
                        "locale": vp.status().locale.raw_locale,
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
        "voice.tts" => {
            let text = req
                .params
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let play = req
                .params
                .get("play")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let (models_dir, _, _) = transmute::resolve_models_dir(&state.data_dir);
            let vp = nos_voice::VoicePipeline::with_models_dir(models_dir);
            match vp.synthesize_wav(text, play) {
                Ok(wav_bytes) => {
                    let b64 = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &wav_bytes,
                    );
                    Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "bytes": wav_bytes.len(),
                            "wav_b64": b64,
                            "voice": vp.status().locale.piper_voice,
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
        "voice.turn" => {
            // Ciclo completo 100% local: STT (Whisper) -> MoE (Cortex/Hermes dGPU) -> TTS (Piper)
            let (models_dir, _, _) = transmute::resolve_models_dir(&state.data_dir);
            let vp = nos_voice::VoicePipeline::with_models_dir(models_dir);
            let transcript = if let Some(p) = req.params.get("prompt").and_then(|v| v.as_str()) {
                if !p.trim().is_empty() {
                    Ok(p.to_string())
                } else {
                    vp.record_and_transcribe(4)
                }
            } else if let Some(b64) = req.params.get("wav_b64").and_then(|v| v.as_str()) {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
                {
                    Ok(bytes) => {
                        let tmp = std::env::temp_dir().join("nosd-turn-stt.wav");
                        let _ = std::fs::write(&tmp, bytes);
                        vp.transcribe_wav_path(&tmp)
                    }
                    Err(e) => Err(nos_voice::VoiceError::Exec(format!("base64: {e}"))),
                }
            } else {
                let secs = req
                    .params
                    .get("record_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(4) as u32;
                let vad = req
                    .params
                    .get("vad")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if vad {
                    vp.record_and_transcribe_vad(secs.max(10))
                } else {
                    vp.record_and_transcribe(secs)
                }
            };

            let transcript = match transcript {
                Ok(t) if !t.trim().is_empty() => t,
                Ok(_) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some("stt empty transcript".into()),
                        result: json!(null),
                    });
                }
                Err(e) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("stt: {e}")),
                        result: json!(null),
                    });
                }
            };

            let route = state.hermes.explain_route(&transcript);
            let reply = match state.hermes.chat(gate, &transcript) {
                Ok(r) => r,
                Err(e) => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("hermes: {e}")),
                        result: json!({ "transcript": transcript }),
                    });
                }
            };

            let play = req
                .params
                .get("play")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let (tts_ok, wav_b64) = match vp.synthesize_wav(&reply, play) {
                Ok(wav) => (
                    true,
                    Some(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &wav,
                    )),
                ),
                Err(_) => (false, None),
            };

            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "transcript": transcript,
                    "reply": reply,
                    "slot": route.slot.name(),
                    "reason": route.reason,
                    "locale": vp.status().locale.raw_locale,
                    "tts_ok": tts_ok,
                    "wav_b64": wav_b64,
                }),
            }
        }
        "transmute.status" => {
            let st =
                transmute::query_status(&state.data_dir, &state.hermes.hub, &state.hermes.hotswap);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(st).unwrap_or(json!(null)),
            }
        }
        "transmute.run" => {
            if !super::hitl_grant_ok(state, &req, "transmute") {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("HITL required: confirm_id from ui.action (kind=transmute)".into()),
                    result: json!({ "need": "confirm_id", "kind": "transmute" }),
                });
            }
            let opts: transmute::TransmuteRunOptions =
                serde_json::from_value(req.params.clone()).unwrap_or_default();
            let res = transmute::execute_transmute(
                &state.data_dir,
                &state.hermes.hub,
                &state.hermes.hotswap,
                opts,
            );
            opar_after_ok(
                state,
                "transmute.run",
                res.ok,
                "transmute_wizard",
                "execute",
                &format!("downloaded={}", res.downloaded.len()),
                if res.ok { "ok" } else { "errors" },
            );
            Response {
                id: req.id,
                ok: res.ok,
                error: if res.ok {
                    None
                } else {
                    Some(res.errors.join("; "))
                },
                result: serde_json::to_value(res).unwrap_or(json!(null)),
            }
        }

        _ => return None,
    })
}
