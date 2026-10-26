use nosd_lib::{actuators, nix_managed, transmute, wifi_nm};
use nos_core::{
    CapRegistry, NosScore, SafetyViolation, Sev, ActDomain, ActDisposition, disposition,
    lab_auto_switch_from_env, note_violation, s1_backend_from_env, s1_backend_label, S1Backend,
    UiSource, WindowSpec,
};
use nos_factory::{
    ImportMeta, ImportProvenance, PackageStore, SandboxStatus, StoreError,
};
use parking_lot::Mutex;
use nos_cortex::{CapacityProber, ModelHub};
use nos_hermes::{
    deliberate_blob_fetch, iso_must_not_ship_blobs, remember_deliberation, BlobKind, BlobNeed,
    HermesEngine,
};
use nos_ipc::{Request, Response, decode_body, encode, take_frame};
use nos_mesh::{fleet_worst, HealthVerdict, MachineHealth, MeshEngine};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn};

pub struct AppState {
    pub hermes: HermesEngine,
    pub mesh: MeshEngine,
    pub hub: ModelHub,
    pub data_dir: PathBuf,
    pub cap_gen: u64,
    /// CapGate server-authored (I.1) — nunca confiar em req.caps.
    pub caps: CapRegistry,
    pub package_store: Mutex<PackageStore>,
    pub shutting_down: AtomicBool,
    /// Contador do loop Continuous Hermes (0 = ainda não tickou / desligado).
    pub hermes_ticks: std::sync::atomic::AtomicU64,
}

fn opar_after_ok(state: &AppState, method: &str, ok: bool, observe: &str, plan: &str, act: &str, verify: &str) {
    if ok && nos_hermes::is_side_effect(method) {
        let _ = state.hermes.opar_side_effect(method, observe, plan, act, verify);
    }
}

fn note_capgate_deny(agent: &str, method: &str, detail: &str) {
    let _ = note_violation(SafetyViolation {
        invariant: "capgate".into(),
        agent: agent.to_string(),
        skill: method.to_string(),
        detail: detail.to_string(),
    });
}

/// W4/W5 — score honesto: S1 label, llama-server posture, landlock, early boot JSON.
fn build_runtime_score(state: &AppState, phase: &str, msg: &str) -> NosScore {
    let fleet_verdict = state
        .mesh
        .fleet_verdict()
        .map(|v| v.as_str().to_string());
    let llama = if nos_cortex::llama_server_base().is_some() {
        format!("url_set+{}", state.hermes.hotswap.engine_name())
    } else {
        format!("absent+{}", state.hermes.hotswap.engine_name())
    };
    let early_boot = {
        let p = state.data_dir.join("boot-score.json");
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    };
    NosScore {
        phase: phase.into(),
        sev: Sev::Ok,
        msg: msg.into(),
        hub_slots: state.hub.loaded_count(),
        memory_ok: state.hermes.memory.health(),
        mesh_peers: state.mesh.peer_count(),
        fleet_verdict,
        summary: String::new(),
        s1_backend: s1_backend_label().into(),
        llama_server: llama,
        landlock: format!("{:?}", nosd_lib::landlock::applied_status()),
        early_boot,
    }
    .with_summary()
}

pub async fn serve_tcp(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    loop {
        if state.shutting_down.load(Ordering::Relaxed) {
            break;
        }
        let (sock, _) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_stream(sock, st).await {
                warn!(target: "nosd", "conn err: {e}");
            }
        });
    }
    Ok(())
}

#[cfg(unix)]
pub async fn serve_unix(path: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = tokio::net::UnixListener::bind(&path)?;
    loop {
        if state.shutting_down.load(Ordering::Relaxed) {
            break;
        }
        let (sock, _) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_stream(sock, st).await {
                warn!(target: "nosd", "conn err: {e}");
            }
        });
    }
    Ok(())
}

fn method_domain(method: &str) -> Option<ActDomain> {
    Some(match method {
        "chat" | "chat.submit" | "route.explain" => ActDomain::ChatRoute,
        "model.plan" | "model.suggest" => ActDomain::ModelPlanSuggest,
        "model.fetch" | "model.search" | "blob.deliberate" => ActDomain::ModelFetch,
        "firmware.fetch" => ActDomain::DeviceFirmware,
        "model.load" => ActDomain::HotSwapKnownHash,
        "wifi.scan" => ActDomain::WifiScan,
        "wifi.connect" => ActDomain::WifiConnect,
        "nix.propose" => ActDomain::NixPropose,
        "nix.apply" | "nix.confirm" | "nix.build_vm" => ActDomain::NixSwitch,
        "nix.rollback" => ActDomain::NixRollback,
        "agent.spawn" | "factory.promote" => ActDomain::AgentSpawnSigned,
        "factory.seek" | "store.search" => ActDomain::FactorySeekScan,
        "store.install" => ActDomain::FactorySeekInstall,
        "factory.synthesize" => ActDomain::FactorySynthesize,
        "systemctl.scoped" => ActDomain::SystemctlScoped,
        "audio.volume" => ActDomain::PipeWireVolume,
        "gpu.plan" => ActDomain::ModelPlanSuggest, // flake/ngl plan = Auto; power-limit = Deny domain
        "gpu.power_limit" => ActDomain::GpuPowerLimit,
        "device.recipe" => ActDomain::NixPropose,
        "mesh.skillsync" => ActDomain::MeshSkillSyncExec,
        _ => return None,
    })
}

async fn handle_stream<S>(mut sock: S, state: Arc<AppState>) -> anyhow::Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut acc = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = sock.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        acc.extend_from_slice(&buf[..n]);
        while let Some((body, consumed)) = take_frame(&acc)? {
            acc.drain(..consumed);
            let req: Request = decode_body(&body)?;
            let resp = dispatch(&state, req);
            sock.write_all(&encode(&resp)?).await?;
        }
    }
    Ok(())
}

/// Atualiza MCH local a partir de sinais runtime (honest partial — não substitui observe.snapshot).
pub fn sync_local_fleet_health(state: &AppState) {
    let ticks = state
        .hermes_ticks
        .load(std::sync::atomic::Ordering::Relaxed);
    let sys = if ticks > 0 {
        HealthVerdict::Go
    } else {
        HealthVerdict::Unknown
    };
    let hub = if state.hub.loaded_count() > 0 {
        HealthVerdict::Go
    } else {
        HealthVerdict::Unknown
    };
    state.mesh.set_local_fleet_health(MachineHealth {
        node_id: state.mesh.local_id().min(255) as u8,
        sys,
        audio: HealthVerdict::Unknown,
        hub,
        reason: format!("hermes_ticks={} hub_loaded={}", ticks, state.hub.loaded_count()),
    });
}

fn dispatch(state: &AppState, req: Request) -> Response {
    // I.1: caps server-authored by agent identity; client bitmask ignored.
    let gate = match state.caps.gate_for(&req.agent, req.caps, state.cap_gen) {
        Ok(g) => g,
        Err(e) => {
            note_capgate_deny(&req.agent, &req.method, &e.to_string());
            return Response {
                id: req.id,
                ok: false,
                error: Some(e.into()),
                result: json!({ "capgate": "server_authored" }),
            };
        }
    };
    let need = req.required_cap();
    if need.0 != 0 {
        if let Err(e) = gate.allow(need, state.cap_gen) {
            note_capgate_deny(&req.agent, &req.method, e);
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("{e} [server-authored agent={}]", req.agent)),
                result: json!({ "need": need.0, "held": gate.held.0 }),
            };
        }
    }
    // NX.0: policy gate for destructive domains (before side-effects).
    if let Some(dom) = method_domain(&req.method) {
        let lab = lab_auto_switch_from_env();
        let disp = disposition(dom, lab);
        if disp == ActDisposition::Deny {
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("SITE_POLICY deny domain={dom:?}")),
                result: json!({ "disposition": "deny" }),
            };
        }
        // ModelFetch / DeviceFirmware: HITL via blob_deliberate (abre card JARBAS) — não short-circuit.
        let blob_domain = matches!(dom, ActDomain::ModelFetch | ActDomain::DeviceFirmware);
        if disp == ActDisposition::Escalate
            && !blob_domain
            && req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) != Some(true)
        {
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("SITE_POLICY escalate domain={dom:?} — HITL required")),
                result: json!({ "disposition": "escalate", "need_hitl": true }),
            };
        }
        if disp == ActDisposition::Confirm
            && matches!(
                dom,
                ActDomain::NixSwitch | ActDomain::WifiConnect | ActDomain::FactorySeekInstall
            )
            && req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) != Some(true)
            && !(lab && matches!(dom, ActDomain::NixSwitch))
        {
            return Response {
                id: req.id,
                ok: false,
                error: Some(format!("SITE_POLICY confirm domain={dom:?}")),
                result: json!({ "disposition": "confirm", "need_hitl": true }),
            };
        }
    }

    match req.method.as_str() {
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
                    "hub_loaded": state.hub.loaded_count(),
                    "engine": state.hermes.hotswap.engine_name(),
                    "llama_server": if nos_cortex::llama_server_base().is_some() { "url_set" } else { "absent" },
                    "s1_backend": s1_backend_label(),
                    "landlock": format!("{:?}", nosd_lib::landlock::applied_status()),
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
                    "honesty": "MCH wire partial — no auto TX on HB; CRDT NOT_DONE",
                }),
            }
        }
        "observe.snapshot" => {
            let snap = nos_observe::collect_snapshot();
            state.hermes.bus.publish(
                nos_observe::TOPIC_OBSERVE_SNAPSHOT,
                nos_observe::snapshot_bus_payload(&snap),
            );
            // NX.5: thermal/presence → env ngl cap (não força llama; hint Continuous).
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
        "model.list" => {
            let list: Vec<_> = state
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
            // Capacity + HF search → recomendação (sem download)
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
            // ADR-0113 / remigração: blobs externos só runtime, deliberados Cortex→Hermes→JARBAS.
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
                return Response {
                    id: req.id,
                    ok: false,
                    error: Some("need params.url or params.repo+file".into()),
                    result: json!({ "iso_policy": iso_must_not_ship_blobs() }),
                };
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
                return Response {
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
                };
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
                        Response {
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
                        }
                    }
                    Err(e) => Response {
                        id: req.id,
                        ok: false,
                        error: Some(e.to_string()),
                        result: json!({ "deliberation": plan }),
                    },
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
                    return Response {
                        id: req.id,
                        ok: false,
                        error: Some("need params.url or params.repo+file".into()),
                        result: json!({ "deliberation": plan }),
                    };
                }
            };
            match dest {
                Ok(path) => {
                    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let slot = nos_cortex::slot_from_path(&path);
                    state.hub.register_path(slot, path.clone(), len);
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
                return Response {
                    id: req.id,
                    ok: false,
                    error: Some("params.path required".into()),
                    result: json!(null),
                };
            }
            let p = PathBuf::from(path);
            match state.hermes.hotswap.hot_load_llama(p.clone()) {
                Ok(()) => {
                    let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    let slot = nos_cortex::slot_from_path(&p);
                    state.hub.register_path(slot, p.clone(), len);
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
        "chat" | "infer.submit" => {
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match state.hermes.chat(&gate, prompt) {
                Ok(text) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "text": text }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
        "chat.submit" => {
            let prompt = req
                .params
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match state.hermes.chat_submit(&gate, prompt) {
                Ok(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: v,
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
        "chat.poll" => {
            let job_id = req
                .params
                .get("id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            match state.hermes.chat_poll(job_id) {
                Some(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: v,
                },
                None => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "id": job_id, "pending": true }),
                },
            }
        }
        "memory.recall" => {
            let q = req
                .params
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match state.hermes.recall_context(q) {
                Ok(texts) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!(texts),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!(null),
                },
            }
        }
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
        "nix.build_vm" => match nix_managed::request_build_vm(&state.data_dir) {
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
        },
        "systemctl.scoped" => {
            let unit = req.params.get("unit").and_then(|v| v.as_str()).unwrap_or("");
            let verb = req
                .params
                .get("verb")
                .and_then(|v| v.as_str())
                .unwrap_or("status");
            let hitl = req.params.get("hitl_confirmed").and_then(|v| v.as_bool()) == Some(true);
            match actuators::systemctl_scoped(&gate, state.cap_gen, unit, verb, hitl) {
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
            match actuators::pipewire_volume_plan(&gate, state.cap_gen, pct, hitl) {
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
            let drm = req
                .params
                .get("drm")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
        }
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
        }
        "wifi.connect" => {
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
                vp.record_and_transcribe(secs)
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
                vp.record_and_transcribe(secs)
            };

            let transcript = match transcript {
                Ok(t) if !t.trim().is_empty() => t,
                Ok(_) => {
                    return Response {
                        id: req.id,
                        ok: false,
                        error: Some("stt empty transcript".into()),
                        result: json!(null),
                    };
                }
                Err(e) => {
                    return Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("stt: {e}")),
                        result: json!(null),
                    };
                }
            };

            let route = state.hermes.explain_route(&transcript);
            let reply = match state.hermes.chat(&gate, &transcript) {
                Ok(r) => r,
                Err(e) => {
                    return Response {
                        id: req.id,
                        ok: false,
                        error: Some(format!("hermes: {e}")),
                        result: json!({ "transcript": transcript }),
                    };
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
            let st = transmute::query_status(&state.data_dir, &state.hub, &state.hermes.hotswap);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: serde_json::to_value(st).unwrap_or(json!(null)),
            }
        }
        "transmute.run" => {
            let opts: transmute::TransmuteRunOptions =
                serde_json::from_value(req.params.clone()).unwrap_or_default();
            let res = transmute::execute_transmute(
                &state.data_dir,
                &state.hub,
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
                state.hermes.ui_open_wifi(&gate, source, ssids)
            } else {
                state.hermes.ui_open(&gate, kind, source, title, body)
            };
            match opened {
                Ok(w) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: serde_json::to_value(&w).unwrap_or(json!(null)),
                },
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
            match state.hermes.ui_action(&gate, act) {
                Ok(mut v) => {
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
                        return Response {
                            id: req.id,
                            ok: false,
                            error: Some(format!("body_b64: {e}")),
                            result: json!(null),
                        };
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
                    error: Some("unsigned — content_hash required".into()),
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
                    return Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "seek_hit": true,
                            "report": serde_json::to_value(&report).unwrap_or(json!(null)),
                            "note": "import candidate — use store.install; synthesize skipped",
                        }),
                    };
                }
                if !report.synthesize_allowed {
                    return Response {
                        id: req.id,
                        ok: false,
                        error: Some(
                            report
                                .synthesize_reason
                                .clone()
                                .unwrap_or_else(|| "Seek incomplete — NetAbsent or pending".into()),
                        ),
                        result: serde_json::to_value(&report).unwrap_or(json!(null)),
                    };
                }
            }
            // omit wasm_b64 → Frugal identity; else decode standard base64 → path A validate
            let wasm_bytes: Vec<u8> = match req.params.get("wasm_b64").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    s.trim(),
                ) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response {
                            id: req.id,
                            ok: false,
                            error: Some(format!("wasm_b64 decode: {e}")),
                            result: json!(null),
                        };
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
        other => Response {
            id: req.id,
            ok: false,
            error: Some(format!("unknown method {other}")),
            result: json!(null),
        },
    }
}

const ORB_LAB_HTML: &str = include_str!("../static/orb_lab.html");
const TRANSMUTE_HTML: &str = include_str!("../static/transmute.html");

/// Lab cinemático: GET `/` → HTML (cam+mic+padrões); GET `/transmute` → wizard conversão; POST `/rpc` → dispatch IPC.
pub async fn serve_orb_lab(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(target: "nosd", "orb lab listening http://{addr}");
    loop {
        if state.shutting_down.load(Ordering::Relaxed) {
            break;
        }
        let (mut sock, _) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_orb_lab_conn(&mut sock, st).await {
                warn!(target: "nosd", "orb lab conn: {e}");
            }
        });
    }
    Ok(())
}

async fn handle_orb_lab_conn<S>(sock: &mut S, state: Arc<AppState>) -> anyhow::Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buf = vec![0u8; 65536];
    let n = sock.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    let first = head.lines().next().unwrap_or("");
    let is_post_rpc = first.starts_with("POST /rpc");
    let is_transmute = first.starts_with("GET /transmute");
    let is_get = first.starts_with("GET / ") || first.starts_with("GET /index") || first.starts_with("GET /?");

    if is_transmute {
        let body = TRANSMUTE_HTML.as_bytes();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
            body.len()
        );
        sock.write_all(resp.as_bytes()).await?;
        sock.write_all(body).await?;
        return Ok(());
    }

    if is_get {
        let body = ORB_LAB_HTML.as_bytes();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
            body.len()
        );
        sock.write_all(resp.as_bytes()).await?;
        sock.write_all(body).await?;
        return Ok(());
    }

    if first.starts_with("OPTIONS ") {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
        sock.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    if !is_post_rpc {
        let msg = b"not found";
        let resp = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            msg.len()
        );
        sock.write_all(resp.as_bytes()).await?;
        sock.write_all(msg).await?;
        return Ok(());
    }

    // Body: may already be in buf after headers
    let header_end = head
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| head.find("\n\n").map(|i| i + 2))
        .unwrap_or(n);
    let mut body = buf[header_end.min(n)..n].to_vec();
    let content_len = head
        .lines()
        .find_map(|l| {
            let l = l.to_ascii_lowercase();
            l.strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    while body.len() < content_len {
        let m = sock.read(&mut buf).await?;
        if m == 0 {
            break;
        }
        body.extend_from_slice(&buf[..m]);
    }
    body.truncate(content_len);

    let req: Request = serde_json::from_slice(&body).unwrap_or(Request {
        id: 0,
        method: "health".into(),
        caps: 0,
        cap_gen: 1,
        agent: "nosctl".into(),
        params: json!({}),
    });
    let resp = dispatch(&state, req);
    let payload = serde_json::to_vec(&resp)?;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        payload.len()
    );
    sock.write_all(header.as_bytes()).await?;
    sock.write_all(&payload).await?;
    Ok(())
}
