//! nosd — Neural-OS Daemon (NixAIOS).

mod server;

use clap::Parser;
use nos_bei::BeiRuntime;
use nos_bus::EventBus;
use nos_cortex::{
    CapacityProber, HotSwapController, ModelHub, S2Queue, scan_path, slot_from_path,
};
use nos_core::{ModelSlot, NosScore, Sev};
use nos_factory::AppFactory;
use nos_hermes::HermesEngine;
use nos_memory::NosMemory;
use nos_mesh::MeshEngine;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "nosd", about = "Neural-OS Daemon (NixAIOS)")]
struct Args {
    #[arg(long, default_value = "/run/nosd/nosd.sock")]
    socket: String,
    /// TCP fallback (Windows / lab): e.g. 127.0.0.1:7700
    #[arg(long)]
    tcp: Option<String>,
    #[arg(long, default_value = "/var/lib/nosd")]
    data_dir: PathBuf,
    #[arg(long)]
    foreground: bool,
    #[arg(long, default_value_t = 15)]
    node_id: u32,
    /// Hermes Continuous tick (segundos). 0 = só IPC (JARBAS). Default 2 = AIOS desde o boot.
    #[arg(long, default_value_t = 2, env = "NOSD_HERMES_TICK_SECS")]
    hermes_tick_secs: u64,
    /// Mesh UDP bind (vazio = mesh off). Ex: `0.0.0.0:19000`
    #[arg(long, default_value = "", env = "NOSD_MESH_BIND")]
    mesh_bind: String,
    /// Destino broadcast/unicast dos heartbeats.
    #[arg(long, default_value = "255.255.255.255:19000", env = "NOSD_MESH_BROADCAST")]
    mesh_broadcast: String,
    /// Lab cinemático orb (vídeo+som+padrões) — HTTP JSON. Vazio = off.
    /// Ex: `127.0.0.1:7701` → abra no browser.
    #[arg(long, default_value = "127.0.0.1:7701", env = "NOSD_ORB_LAB")]
    orb_lab: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    std::fs::create_dir_all(&args.data_dir)?;
    let mem_dir = args.data_dir.join("nsgdb");
    let models_dir = args.data_dir.join("models");
    std::fs::create_dir_all(&models_dir)?;

    let cap = CapacityProber::probe();
    let posture = nos_cortex::CapacityProber::vram_posture(&cap);
    // Sugestão honesta: só útil com llama CUDA; default nixpkgs = CPU (ngl=0).
    let suggest_ngl = nos_cortex::plan_n_gpu_layers(
        32,
        80,
        cap.vram_free_mb,
        256,
        128,
        256,
    );
    info!(
        target: "nosd",
        "capacity ram_avail_mb={} vendor={} cards={:?} render={:?} vram_free_mb={} posture={} suggest_ngl={} (CUDA llama only; NOSD_LLAMA_NGL overrides)",
        cap.ram_avail_mb,
        cap.vendor_hint,
        cap.dri_cards,
        cap.dri_render,
        cap.vram_free_mb,
        posture,
        suggest_ngl
    );

    if suggest_ngl > 0 && std::env::var("NOSD_LLAMA_NGL").is_err() {
        std::env::set_var("NOSD_LLAMA_NGL", suggest_ngl.to_string());
    }

    // I.3: Landlock real quando NOSD_LANDLOCK=1 (Linux); senão SoftAllowlist honesty.
    let ll = nosd_lib::landlock::try_restrict_process(&args.data_dir);
    info!(target: "nosd", "landlock status={ll:?}");

    let (resolved_models, persist_mounted, _) = nosd_lib::transmute::resolve_models_dir(&args.data_dir);
    let scan_dirs = if persist_mounted && resolved_models != models_dir {
        vec![resolved_models, models_dir.clone()]
    } else {
        vec![models_dir.clone()]
    };

    let hub = ModelHub::new();
    let mut first_gguf: Option<std::path::PathBuf> = None;
    for dir in scan_dirs {
        for meta in scan_path(&dir, 64) {
            if meta.kind != "gguf" {
                continue;
            }
            let len = std::fs::metadata(&meta.path).map(|m| m.len()).unwrap_or(0);
            let slot = slot_from_path(&meta.path);
            hub.register_path(slot, meta.path.clone(), len);
            info!(
                target: "nosd",
                "hub register {} {}",
                slot.name(),
                meta.path.display()
            );
            if first_gguf.is_none() {
                first_gguf = Some(meta.path.clone());
            }
        }
    }

    let hotswap = HotSwapController::new_cpu();
    let attach = hub.path_of(ModelSlot::Active).or(first_gguf);
    if let Some(path) = attach {
        match hotswap.try_attach_llama(path) {
            Ok(()) => info!(target: "nosd", "engine={}", hotswap.engine_name()),
            Err(e) => info!(
                target: "nosd",
                "engine=cpu-echo ({e}) — set NOSD_LLAMA_CLI + models/*.gguf for GGUF"
            ),
        }
    } else {
        info!(target: "nosd", "engine=cpu-echo (no gguf in {})", models_dir.display());
    }

    let hs_s2 = hotswap.clone();
    let s2 = S2Queue::start(std::sync::Arc::new(move |p| {
        hs_s2.infer(p).map_err(|e| e.to_string())
    }));

    let memory = NosMemory::open(&mem_dir)?;
    let bus = EventBus::new(64);
    let hermes = HermesEngine {
        hub: hub.clone(),
        hotswap,
        s2,
        memory: memory.clone(),
        bei: BeiRuntime::new(10_000),
        windows: nos_hermes::WindowBroker::new(bus.clone()),
        orb: nos_hermes::OrbBroker::new(bus.clone()),
        latent: nos_bus::LatentBus::new(),
        agency: parking_lot::Mutex::new(nos_hermes::AgencyFleet::new()),
        skill_observer: nos_hermes::SkillObserver::new(),
        mesh_offers: parking_lot::Mutex::new(Vec::new()),
        federated_index: parking_lot::Mutex::new(nos_factory::FederatedIndex::new()),
        autolearn_rot: std::sync::atomic::AtomicU64::new(0),
        bus,
        factory: AppFactory::new(),
        cap_gen: 1,
        chat_budget: parking_lot::Mutex::new(nos_core::IterationBudget::new(256)),
        persona: parking_lot::Mutex::new(nos_bei::PersonaState::default()),
        idle_ticks: std::sync::atomic::AtomicU64::new(0),
    };
    // Seed cinematic: orb + janela Info (HUD nunca “vazio” de significado)
    let _ = hermes.orb.set_state(nos_core::OrbState::Connecting, true);
    let gate_boot = nos_core::CapGate::new(nos_core::CapBits::FULL, 1);
    let _ = hermes.ui_open(
        &gate_boot,
        nos_core::WindowKind::Info,
        nos_core::UiSource::Hermes,
        Some("JARBAS".into()),
        Some("Sistema online — orb cinemático · diga oi ou peça Wi‑Fi.".into()),
    );
    let _ = hermes.orb.set_state(nos_core::OrbState::Idle, true);
    let mesh = MeshEngine::new(args.node_id);

    let score = NosScore {
        phase: "ready".into(),
        sev: Sev::Ok,
        msg: "nosd scaffold up".into(),
        hub_slots: hub.loaded_count(),
        memory_ok: memory.health(),
        mesh_peers: mesh.peer_count(),
        fleet_verdict: None,
        summary: String::new(),
        s1_backend: nos_core::s1_backend_label().into(),
        llama_server: if nos_cortex::llama_server_base().is_some() {
            "url_set".into()
        } else {
            "absent".into()
        },
        landlock: format!("{:?}", nosd_lib::landlock::applied_status()),
        early_boot: None,
    }
    .with_summary();
    info!(target: "nosd", "{}", score.line());

    // sd_notify READY=1 when NOTIFY_SOCKET set (systemd)
    sd_notify_ready();

    let state = Arc::new(server::AppState {
        hermes,
        mesh,
        hub,
        data_dir: args.data_dir.clone(),
        cap_gen: 1,
        caps: nos_core::CapRegistry::new(),
        package_store: parking_lot::Mutex::new(nos_factory::PackageStore::new()),
        shutting_down: AtomicBool::new(false),
        hermes_ticks: std::sync::atomic::AtomicU64::new(0),
    });

    let mesh_udp = if !args.mesh_bind.is_empty() {
        match nos_mesh::udp::start_mesh_udp(&args.mesh_bind, &args.mesh_broadcast, state.mesh.clone())
            .await
        {
            Ok(u) => {
                info!(
                    target: "nosd",
                    "mesh udp ON bind={} broadcast={}",
                    args.mesh_bind,
                    args.mesh_broadcast
                );
                Some(u)
            }
            Err(e) => {
                tracing::warn!(target: "nosd", "mesh udp bind failed ({e}) — mesh off");
                None
            }
        }
    } else {
        info!(target: "nosd", "mesh udp OFF (set NOSD_MESH_BIND=0.0.0.0:19000)");
        None
    };

    // Hermes Continuous — não depende do JARBAS; AIOS desde o boot do nosd
    if args.hermes_tick_secs > 0 {
        let st = state.clone();
        let every = args.hermes_tick_secs;
        let mesh_u = mesh_udp.clone();
        tokio::spawn(async move {
            hermes_runtime_loop(st, every, mesh_u).await;
        });
        info!(
            target: "nosd",
            "hermes continuous tick every {}s (NOSD_HERMES_TICK_SECS)",
            every
        );
    } else {
        info!(target: "nosd", "hermes continuous OFF (tick_secs=0) — só IPC");
    }

    if !args.orb_lab.is_empty() {
        let lab = args.orb_lab.clone();
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = server::serve_orb_lab(lab, st).await {
                tracing::warn!(target: "nosd", "orb lab http failed: {e}");
            }
        });
        info!(
            target: "nosd",
            "orb cinematic lab http://{}  (cam+mic+patterns → orb.sense)",
            args.orb_lab
        );
    }

    let bind = args.tcp.clone();
    #[cfg(unix)]
    {
        if bind.is_none() {
            let _ = std::fs::remove_file(&args.socket);
            info!(target: "nosd", "listen unix {}", args.socket);
            server::serve_unix(args.socket.clone(), state.clone()).await?;
            return Ok(());
        }
    }
    let addr = bind.unwrap_or_else(|| "127.0.0.1:7700".into());
    info!(target: "nosd", "listen tcp {}", addr);
    server::serve_tcp(addr, state).await?;
    Ok(())
}

/// Loop Observe liveness do Hermes (BEI + S2 drain + mesh HB) — HermesAgent Continuous.
async fn hermes_runtime_loop(
    state: Arc<server::AppState>,
    every_secs: u64,
    mesh_udp: Option<std::sync::Arc<nos_mesh::udp::MeshUdp>>,
) {
    let mut n = 0u64;
    let period = std::time::Duration::from_secs(every_secs.max(1));
    loop {
        if state.shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let tick = state.hermes.cognitive_tick();
        // NX.3/NX.5: observe → EventBus + thermal/presence ngl hint
        if n % 3 == 0 {
            let snap = nos_observe::collect_snapshot();
            state.hermes.bus.publish(
                nos_observe::TOPIC_OBSERVE_SNAPSHOT,
                nos_observe::snapshot_bus_payload(&snap),
            );
            if snap.presence.prefer_vram_release() || snap.ngl_reduce {
                let cap = snap.suggest_ngl_cap;
                let cur = std::env::var("NOSD_LLAMA_NGL")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(99);
                if cap < cur {
                    std::env::set_var("NOSD_LLAMA_NGL", cap.to_string());
                    tracing::info!(
                        target: "nosd",
                        "NX.5 ngl cap={} presence={:?} thermal={}",
                        cap,
                        snap.presence,
                        snap.ngl_reduce
                    );
                }
            }
        }
        // Drain S2 results prontos (fire-and-forget) → EventBus
        for r in state.hermes.s2.take_ready(8) {
            state.hermes.bus.publish(
                "HERMES_S2_DONE",
                serde_json::json!({
                    "id": r.id,
                    "ok": r.ok,
                    "text_len": r.text.len(),
                }),
            );
        }
        if let Some(ref u) = mesh_udp {
            // M.2/M.4: LayaIntent outbound + S2 offload peer hint
            if n % 5 == 0 {
                use nos_mesh::{content_fingerprint, LayaIntent};
                state.mesh.queue_laya_intent(LayaIntent {
                    node_id: state.mesh.local_id(),
                    clock: n,
                    intent_arm: if state.hermes.s2.pending() > 0 { 2 } else { 1 },
                    confidence_q8: 160,
                    margin_q8: 16,
                    text_fnv: content_fingerprint("hermes_tick"),
                });
                if let Some(peer) =
                    nos_cortex::assign_from_mesh_tuples(&state.mesh.peers_for_offload())
                {
                    state.hermes.bus.publish(
                        "MESH_OFFLOAD_HINT",
                        serde_json::json!({ "peer_id": peer, "s2_pending": state.hermes.s2.pending() }),
                    );
                }
            }
            u.tick_tx().await;
            u.poll_rx(8).await;
            if n % 15 == 0 {
                let peers = state.mesh.peer_count();
                let _ = state.hermes.record_opar(
                    "mesh",
                    &format!("tick={n} peers={peers}"),
                    "udp_heartbeat",
                    "tx_rx_poll",
                    "light",
                );
            }
        }
        n = n.wrapping_add(1);
        state
            .hermes_ticks
            .store(n, std::sync::atomic::Ordering::Relaxed);
        if state.mesh.peer_count() > 0 {
            server::sync_local_fleet_health(state.as_ref());
            state.hermes.fleet_hud_from_mesh(
                state.mesh.fleet_verdict(),
                state.mesh.peer_count(),
                state.mesh.fleet_machines().len() as u32,
            );
        }
        if n == 1 || n % 30 == 0 {
            info!(
                target: "nosd",
                "hermes runtime tick={} peers={} payload={}",
                n,
                state.mesh.peer_count(),
                tick
            );
        }
        tokio::time::sleep(period).await;
    }
}

fn sd_notify_ready() {
    let Ok(sock) = std::env::var("NOTIFY_SOCKET") else {
        return;
    };
    // Minimal READY=1 without libsystemd — best-effort Unix datagram.
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixDatagram;
        let path = if let Some(rest) = sock.strip_prefix('@') {
            // abstract namespace — skip in scaffold
            tracing::warn!(target: "nosd", "NOTIFY_SOCKET abstract unsupported in scaffold");
            let _ = rest;
            return;
        } else {
            sock
        };
        if let Ok(d) = UnixDatagram::unbound() {
            let _ = d.send_to(b"READY=1\n", path);
            info!(target: "nosd", "sd_notify READY=1");
        }
    }
    #[cfg(not(unix))]
    {
        let _ = sock;
    }
}
