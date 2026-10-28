//! HUD pixels via Slint (feature `slint-ui`). Cage Wayland / lab winit.
//! Honesty: compila ≠ DRM metal VERIFIED.

use crate::rpc::{next_req, Endpoint};
use nos_ipc::Request;
use std::time::Duration;
use tracing::{info, warn};

slint::include_modules!();

pub fn run(endpoint: Endpoint, interval_secs: u64) -> anyhow::Result<()> {
    // Fallback Slint: só força software se pedido (cinemático default = Chromium+iGPU)
    if std::env::var("JARBAS_FORCE_SOFTWARE").ok().as_deref() == Some("1")
        && std::env::var_os("SLINT_BACKEND").is_none()
    {
        std::env::set_var("SLINT_BACKEND", "winit-software");
    }
    info!(
        target: "jarbas",
        "JARBAS UI start backend={:?} wayland={:?} display={:?}",
        std::env::var("SLINT_BACKEND"),
        std::env::var("WAYLAND_DISPLAY"),
        std::env::var("DISPLAY"),
    );

    let ui = HudWindow::new()?;
    // Sem set_fullscreen: cage maximiza; fullscreen nativo → ecrã preto em alguns wlroots
    ui.show()?;
    info!(target: "jarbas", "JARBAS Slint HUD shown — Alt+F7 / journalctl -u jarbas-cage");

    // Pixels imediatos — não esperar o 1º poll
    ui.set_status("ONLINE".into());
    ui.set_orb_state("idle".into());
    ui.set_orb_pattern("sphere".into());
    ui.set_orb_level_pct(16);
    ui.set_health_line("a ligar nosd…".into());
    ui.set_last_reply("JARBAS · Slint — diga oi, peça estado ou Wi‑Fi.".into());
    ui.set_hub_ok(false);
    // Empurra estado no hub (cinema + sense)
    let _ = endpoint.rpc_blocking(Request {
        id: 1,
        method: "orb.set".into(),
        caps: 0,
        cap_gen: 1,
        agent: "jarbas".into(), token: None,
        params: serde_json::json!({ "state": "idle", "presets": true, "particles": 160, "rings": 3 }),
    });

    // O orb lab publica os sinais sensoriais a cada 200 ms; manter esta leitura
    // separada da telemetria lenta para que áudio/câmara não esperem score/health.
    let ui_orb = ui.as_weak();
    let ep_orb = endpoint.clone();
    std::thread::spawn(move || {
        let mut id = 1u64;
        loop {
            let orb = ep_orb.rpc_blocking(next_req(&mut id, "orb.get", 0, serde_json::json!({})));
            let ui_w = ui_orb.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_w.upgrade() {
                    match &orb {
                        Ok(r) if r.ok => {
                            if let Some(s) = r.result.get("state").and_then(|v| v.as_str()) {
                                ui.set_orb_state(s.into());
                            }
                            if let Some(s) = r.result.get("pattern").and_then(|v| v.as_str()) {
                                ui.set_orb_pattern(s.into());
                            }
                            let sense = r.result.get("sense");
                            let level = sense
                                .and_then(|s| s.get("level"))
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let motion = sense
                                .and_then(|s| s.get("motion"))
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let audio_on = sense
                                .and_then(|s| s.get("audio_on"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let video_on = sense
                                .and_then(|s| s.get("video_on"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            ui.set_orb_level_pct((level * 100.0).clamp(0.0, 100.0) as i32);
                            ui.set_orb_motion_pct((motion * 100.0).clamp(0.0, 100.0) as i32);
                            ui.set_orb_audio_on(audio_on);
                            ui.set_orb_video_on(video_on);
                        }
                        _ => {
                            ui.set_orb_level_pct(0);
                            ui.set_orb_motion_pct(0);
                            ui.set_orb_audio_on(false);
                            ui.set_orb_video_on(false);
                        }
                    }
                }
            });
            std::thread::sleep(Duration::from_millis(200));
        }
    });

    let ui_poll = ui.as_weak();
    let ep_poll = endpoint.clone();
    let interval = interval_secs.max(1);

    std::thread::spawn(move || {
        let mut id = 1u64;
        loop {
            let health =
                ep_poll.rpc_blocking(next_req(&mut id, "health", 0, serde_json::json!({})));
            let score = ep_poll.rpc_blocking(next_req(&mut id, "score", 0, serde_json::json!({})));
            let tick = ep_poll.rpc_blocking(next_req(
                &mut id,
                "cognitive.tick",
                0,
                serde_json::json!({}),
            ));
            // Conector janelas HITL — JARBAS materializa o que Hermes/Cortex pediram
            let wins = ep_poll.rpc_blocking(next_req(&mut id, "ui.list", 0, serde_json::json!({})));

            let ui_w = ui_poll.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_w.upgrade() {
                    let mut hitl_id: i32 = 0;
                    let mut hitl_kind = String::new();
                    let mut status_extra = String::new();
                    if let Ok(r) = &wins {
                        if r.ok {
                            if let Some(arr) = r.result.as_array() {
                                if let Some(w0) = arr.first() {
                                    let title = w0
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("janela");
                                    let kind =
                                        w0.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                                    let wid = w0.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let escalate = w0
                                        .get("escalate")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false);
                                    if let Some(body) = w0.get("body").and_then(|v| v.as_str()) {
                                        if !body.is_empty() {
                                            ui.set_last_reply(body.into());
                                        }
                                    }
                                    // Só kinds HITL abrem formulário; Info/Status = cinema + texto
                                    if escalate
                                        || kind.contains("hitl")
                                        || kind == "hitl_wifi"
                                        || kind == "hitl_confirm"
                                        || kind == "hitl_text"
                                    {
                                        hitl_id = wid as i32;
                                        hitl_kind = kind.to_string();
                                        status_extra = format!("HITL · {title}");
                                    } else {
                                        // Janela informativa: não repetir o nome da app.
                                        if !title.eq_ignore_ascii_case("jarbas") {
                                            status_extra = title.to_string();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ui.set_hitl_window_id(hitl_id);
                    ui.set_hitl_kind(hitl_kind.into());
                    match &health {
                        Ok(r) if r.ok => {
                            ui.set_hub_ok(true);
                            let eng = r
                                .result
                                .get("engine")
                                .and_then(|v| v.as_str())
                                .unwrap_or("—");
                            ui.set_engine(eng.into());
                            if let Some(n) = r.result.get("hub_loaded").and_then(|v| v.as_u64()) {
                                ui.set_hub_loaded(n as i32);
                            }
                            if let Some(n) = r.result.get("s2_pending").and_then(|v| v.as_u64()) {
                                ui.set_s2_pending(n as i32);
                            }
                            if let Some(n) = r.result.get("mesh_peers").and_then(|v| v.as_u64()) {
                                ui.set_mesh_peers(n as i32);
                            }
                            ui.set_health_line(format!("ok · {eng}").into());
                            if status_extra.is_empty() {
                                ui.set_status("ONLINE".into());
                            } else {
                                ui.set_status(status_extra.into());
                            }
                        }
                        Ok(r) => {
                            ui.set_hub_ok(false);
                            ui.set_health_line(format!("err {:?}", r.error).into());
                            ui.set_status("DEGRADED".into());
                        }
                        Err(e) => {
                            ui.set_hub_ok(false);
                            ui.set_health_line(format!("nosd: {e}").into());
                            ui.set_status("OFFLINE".into());
                            ui.set_orb_state("degraded".into());
                        }
                    }
                    if let Ok(r) = &score {
                        if r.ok {
                            let s = r
                                .result
                                .get("summary")
                                .or_else(|| r.result.get("line"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if !s.is_empty() {
                                ui.set_score_line(s.into());
                            } else {
                                ui.set_score_line(
                                    r.result
                                        .to_string()
                                        .chars()
                                        .take(96)
                                        .collect::<String>()
                                        .into(),
                                );
                            }
                        }
                    }
                    if let Ok(r) = &tick {
                        if r.ok {
                            if let Some(p) = r.result.get("phase").and_then(|v| v.as_str()) {
                                ui.set_tick_line(p.into());
                            }
                        }
                    }
                }
            });

            std::thread::sleep(Duration::from_secs(interval));
        }
    });

    let ui_chat = ui.as_weak();
    let ep_chat = endpoint.clone();
    ui.on_submit_chat(move |prompt| {
        let prompt = prompt.to_string();
        if prompt.trim().is_empty() {
            return;
        }
        info!(target: "jarbas", "slint chat len={}", prompt.len());
        if let Some(ui) = ui_chat.upgrade() {
            ui.set_orb_state("thinking".into());
            ui.set_status("THINK".into());
            ui.set_last_reply("a pensar…".into());
        }
        let _ = ep_chat.rpc_blocking(Request {
            id: 9000,
            method: "orb.set".into(),
            caps: 0,
            cap_gen: 1,
            agent: "jarbas".into(),
            token: None,
            params: serde_json::json!({ "state": "thinking", "presets": true }),
        });
        // Long / complex → async submit+poll; short → sync chat
        let use_async = prompt.len() > 80;
        let text = if use_async {
            match chat_submit_poll(&ep_chat, &prompt) {
                Ok(t) => t,
                Err(e) => {
                    warn!(target: "jarbas", "async chat {e}");
                    format!("err: {e}")
                }
            }
        } else {
            let req = Request {
                id: 9001,
                method: "chat".into(),
                caps: 1 | 16,
                cap_gen: 1,
                agent: "jarbas".into(),
                token: None,
                params: serde_json::json!({ "prompt": prompt }),
            };
            match ep_chat.rpc_blocking(req) {
                Ok(r) if r.ok => r
                    .result
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| r.result.to_string()),
                Ok(r) => format!("err {:?}", r.error),
                Err(e) => format!("unreachable {e}"),
            }
        };
        let _ = ep_chat.rpc_blocking(Request {
            id: 9004,
            method: "orb.set".into(),
            caps: 0,
            cap_gen: 1,
            agent: "jarbas".into(),
            token: None,
            params: serde_json::json!({ "state": "speaking", "presets": true }),
        });
        if let Some(ui) = ui_chat.upgrade() {
            ui.set_orb_state("speaking".into());
            ui.set_orb_level_pct(55);
            ui.set_status("SPEAK".into());
            ui.set_last_reply(text.into());
        }
        let ep_idle = ep_chat.clone();
        let ui_idle = ui_chat.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            let _ = ep_idle.rpc_blocking(Request {
                id: 9005,
                method: "orb.set".into(),
                caps: 0,
                cap_gen: 1,
                agent: "jarbas".into(),
                token: None,
                params: serde_json::json!({ "state": "idle", "presets": true }),
            });
            let ui_w = ui_idle;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_w.upgrade() {
                    ui.set_orb_state("idle".into());
                    ui.set_orb_level_pct(12);
                    ui.set_status("ONLINE".into());
                }
            });
        });
    });

    // HITL — CapBits::HITL (1<<6); applied=false até NetworkManager
    let ui_hitl = ui.as_weak();
    let ep_hitl = endpoint.clone();
    ui.on_hitl_submit(move |wid, packed| {
        let packed = packed.to_string();
        let (ssid, password) = match packed.split_once('\n') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (packed, String::new()),
        };
        let mut values = serde_json::Map::new();
        if !ssid.is_empty() {
            values.insert("ssid".into(), serde_json::json!(ssid));
            values.insert("text".into(), serde_json::json!(ssid));
        }
        if !password.is_empty() {
            values.insert("password".into(), serde_json::json!(password));
        }
        let req = Request {
            id: 9101,
            method: "ui.action".into(),
            caps: 1 << 6,
            cap_gen: 1,
            agent: "jarbas".into(),
            token: None,
            params: serde_json::json!({
                "window_id": wid as u64,
                "action": "submit",
                "values": values,
            }),
        };
        let msg = match ep_hitl.rpc_blocking(req) {
            Ok(r) if r.ok => {
                let applied = r
                    .result
                    .get("applied")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let reason = r
                    .result
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ok");
                format!("HITL submit accepted applied={applied} ({reason})")
            }
            Ok(r) => format!("HITL err {:?}", r.error),
            Err(e) => format!("HITL unreachable {e}"),
        };
        info!(target: "jarbas", "{msg}");
        if let Some(ui) = ui_hitl.upgrade() {
            ui.set_last_reply(msg.into());
            ui.set_hitl_window_id(0);
        }
    });

    ui.on_open_orb_lab(|| {
        let url = "http://127.0.0.1:7701/";
        info!(target: "jarbas", "open orb lab {url}");
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            let _ = url;
        }
    });

    let ui_cancel = ui.as_weak();
    let ep_cancel = endpoint;
    ui.on_hitl_cancel(move |wid| {
        let req = Request {
            id: 9102,
            method: "ui.action".into(),
            caps: 1 << 6,
            cap_gen: 1,
            agent: "jarbas".into(),
            token: None,
            params: serde_json::json!({
                "window_id": wid as u64,
                "action": "cancel",
                "values": {},
            }),
        };
        let msg = match ep_cancel.rpc_blocking(req) {
            Ok(r) if r.ok => "HITL cancelado".to_string(),
            Ok(r) => format!("HITL cancel err {:?}", r.error),
            Err(e) => format!("HITL unreachable {e}"),
        };
        if let Some(ui) = ui_cancel.upgrade() {
            ui.set_last_reply(msg.into());
            ui.set_hitl_window_id(0);
        }
    });

    ui.run()?;
    Ok(())
}

fn chat_submit_poll(ep: &Endpoint, prompt: &str) -> anyhow::Result<String> {
    let req = Request {
        id: 9002,
        method: "chat.submit".into(),
        caps: 1 | 16,
        cap_gen: 1,
        agent: "jarbas".into(),
        token: None,
        params: serde_json::json!({ "prompt": prompt }),
    };
    let r = ep.rpc_blocking(req)?;
    if !r.ok {
        anyhow::bail!("{:?}", r.error);
    }
    if r.result.get("ready").and_then(|v| v.as_bool()) == Some(true) {
        return Ok(r
            .result
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string());
    }
    let job_id = r
        .result
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("no job id"))?;
    for _ in 0..200 {
        let poll = Request {
            id: 9003,
            method: "chat.poll".into(),
            caps: 1,
            cap_gen: 1,
            agent: "jarbas".into(),
            token: None,
            params: serde_json::json!({ "id": job_id }),
        };
        let pr = ep.rpc_blocking(poll)?;
        if pr.result.get("pending").and_then(|v| v.as_bool()) != Some(true) {
            if pr.result.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                anyhow::bail!(
                    "{}",
                    pr.result
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("fail")
                );
            }
            return Ok(pr
                .result
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!("poll timeout id={job_id}")
}
