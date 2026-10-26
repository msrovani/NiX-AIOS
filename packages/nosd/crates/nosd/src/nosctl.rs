//! nosctl — cliente CLI do nosd.

use clap::{Parser, Subcommand};
use nos_ipc::{decode_body, encode, take_frame, Request, Response};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/run/nosd/nosd.sock")]
    socket: String,
    #[arg(long)]
    tcp: Option<String>,
    /// Token IPC para TCP (env NOSD_IPC_TOKEN).
    #[arg(long, env = "NOSD_IPC_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// Ficheiro do token IPC (default: /run/nosd/ipc.token, /var/lib/nosd/ipc.token ou ./data/ipc.token).
    #[arg(long, env = "NOSD_IPC_TOKEN_FILE")]
    token_file: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Health,
    Score,
    /// NX.3 / early boot: snapshot journal/udev/NM/hwmon → EventBus OBSERVE_SNAPSHOT.
    Observe,
    /// ModelHub slots registados (path/loaded).
    Models,
    /// Capacity → quant/ngl hint (sem download).
    Suggest,
    /// Capacity + busca HF GGUF → plano recomendado.
    Plan,
    /// Busca HF Hub (filter gguf).
    Search {
        query: String,
        #[arg(long, default_value_t = 8)]
        limit: u32,
    },
    /// HTTPS / HF resolve → /var/lib/nosd/models (CapGate NET|FS).
    /// Requer deliberação Cortex/Hermes + `--confirm` (JARBAS HITL) para download.
    Fetch {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        file: Option<String>,
        #[arg(long)]
        filename: Option<String>,
        /// Confirmação HITL JARBAS (obrigatória p/ Act download).
        #[arg(long)]
        confirm: bool,
        #[arg(long, default_value_t = 500)]
        estimated_mb: u64,
    },
    /// Hot-load GGUF path (unload + llama attach).
    Load {
        path: String,
    },
    /// Unload → cpu-echo.
    Unload,
    Chat {
        prompt: String,
    },
    /// Async: S2 enfileira; S1 devolve ready+text. Poll com `poll`.
    #[command(name = "submit")]
    Submit {
        prompt: String,
    },
    #[command(name = "poll")]
    Poll {
        id: u64,
    },
    Recall {
        query: String,
    },
    /// MoE S1/S2 route explain (no infer).
    Route {
        prompt: String,
    },
    /// Factory A: omit --wasm-b64 → Frugal identity; else path A validate+promote.
    Spawn {
        #[arg(default_value = "agent")]
        name: String,
        #[arg(long)]
        wasm_b64: Option<String>,
    },
    Tick,
    /// ADR-0052 frontmatter deny-by-default (texto MD).
    #[command(name = "artifact-validate")]
    ArtifactValidate {
        md: String,
    },
    /// Janelas JARBAS (conector HITL).
    #[command(name = "ui-list")]
    UiList,
    /// Card HITL thin (ADR-0058): title + kv opcional `k=v,k2=v2`.
    #[command(name = "ui-card")]
    UiCard {
        #[arg(default_value = "HITL")]
        title: String,
        #[arg(long, default_value = "")]
        kv: String,
    },
    #[command(name = "ui-open")]
    UiOpen {
        #[arg(default_value = "hitl_wifi")]
        kind: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body: Option<String>,
    },
    #[command(name = "ui-close")]
    UiClose {
        id: u64,
    },
    #[command(name = "ui-action")]
    UiAction {
        window_id: u64,
        #[arg(default_value = "submit")]
        action: String,
        #[arg(long)]
        ssid: Option<String>,
        #[arg(long)]
        password: Option<String>,
    },
    /// Orb usabilidade + sense (lab cam/mic).
    #[command(name = "orb-get")]
    OrbGet,
    #[command(name = "orb-set")]
    OrbSet {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        pattern: Option<String>,
    },
    /// CapGate NIX_WRITE → staging (+ auto-apply se NOSD_NIX_AUTO_SWITCH=1).
    NixPropose {
        nix: String,
    },
    /// Arma apply root (nosd-nix-apply.service). Exige `--confirm` (admin).
    NixApply {
        #[arg(long)]
        confirm: bool,
    },
    /// Rotaciona token TCP/HTTP + geração (LocalAdmin). Devolve o novo token.
    #[command(name = "security-rotate")]
    SecurityRotate,
    /// Scan Wi-Fi (nmcli).
    WifiScan,
    /// Liga SSID (+ senha). CapGate HITL.
    WifiConnect {
        ssid: String,
        #[arg(long, default_value = "")]
        password: String,
        /// Confirmação HITL (admin). Em JARBAS use ui.action/confirm_id.
        #[arg(long)]
        confirm: bool,
    },
    /// Estado STT (Whisper) + TTS (Piper) + idioma detectado.
    #[command(name = "voice-status")]
    VoiceStatus,
    /// Turno completo 100% local: STT (Whisper) -> MoE (Cortex) -> TTS (Piper).
    #[command(name = "voice-turn")]
    VoiceTurn {
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, default_value_t = 4)]
        record_secs: u32,
    },
    /// Estado da máquina para transmutação (NixOS Gráfico -> AIOS Full).
    #[command(name = "transmute-status")]
    TransmuteStatus,
    /// Executa conversão completa: baixa modelos MoE+Whisper+Piper (runtime, pós-HITL) e Hotswap.
    Transmute {
        #[arg(long)]
        no_download: bool,
        #[arg(long)]
        no_hotswap: bool,
        #[arg(long)]
        locale: Option<String>,
        /// JARBAS Confirm obrigatório se for descarregar blobs externos.
        #[arg(long)]
        confirm: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (method, params, caps) = match &args.cmd {
        Cmd::Health => ("health", serde_json::json!({}), 0u32),
        Cmd::Score => ("score", serde_json::json!({}), 0),
        Cmd::Observe => ("observe.snapshot", serde_json::json!({}), 0),
        Cmd::Models => ("model.list", serde_json::json!({}), 0),
        Cmd::Suggest => ("model.suggest", serde_json::json!({}), 0),
        Cmd::Plan => ("model.plan", serde_json::json!({}), 0),
        Cmd::Search { query, limit } => (
            "model.search",
            serde_json::json!({ "query": query, "limit": limit }),
            (1 << 1) | (1 << 2),
        ),
        Cmd::Fetch {
            url,
            repo,
            file,
            filename,
            confirm,
            estimated_mb,
        } => {
            let mut p = serde_json::json!({
                "hitl_confirmed": confirm,
                "estimated_mb": estimated_mb,
            });
            if let Some(u) = url {
                p["url"] = serde_json::json!(u);
            }
            if let Some(r) = repo {
                p["repo"] = serde_json::json!(r);
            }
            if let Some(f) = file {
                p["file"] = serde_json::json!(f);
            }
            if let Some(n) = filename {
                p["filename"] = serde_json::json!(n);
            }
            // NET|FS
            ("model.fetch", p, (1 << 1) | (1 << 2))
        }
        Cmd::Load { path } => (
            "model.load",
            serde_json::json!({ "path": path }),
            1 | (1 << 2), // INFER|FS
        ),
        Cmd::Unload => ("model.unload", serde_json::json!({}), 1),
        Cmd::Chat { prompt } => ("chat", serde_json::json!({ "prompt": prompt }), 1 | 16), // INFER|MEMORY
        Cmd::Submit { prompt } => (
            "chat.submit",
            serde_json::json!({ "prompt": prompt }),
            1 | 16,
        ),
        Cmd::Poll { id } => ("chat.poll", serde_json::json!({ "id": id }), 1),
        Cmd::Recall { query } => ("memory.recall", serde_json::json!({ "query": query }), 16),
        Cmd::Route { prompt } => ("route.explain", serde_json::json!({ "prompt": prompt }), 0),
        Cmd::Spawn { name, wasm_b64 } => {
            let mut p = serde_json::json!({ "name": name });
            if let Some(b) = wasm_b64 {
                p["wasm_b64"] = serde_json::json!(b);
            }
            ("agent.spawn", p, 1 << 3) // AGENT_SPAWN
        }
        Cmd::Tick => ("cognitive.tick", serde_json::json!({}), 0),
        Cmd::ArtifactValidate { md } => ("artifact.validate", serde_json::json!({ "md": md }), 0),
        Cmd::UiList => ("ui.list", serde_json::json!({}), 0),
        Cmd::UiCard { title, kv } => {
            let mut pairs = Vec::new();
            for part in kv.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if let Some((k, v)) = part.split_once('=') {
                    pairs.push(serde_json::json!({ "key": k.trim(), "value": v.trim() }));
                }
            }
            (
                "ui.card",
                serde_json::json!({ "title": title, "kv": pairs }),
                0,
            )
        }
        Cmd::UiOpen { kind, title, body } => {
            let mut p = serde_json::json!({ "kind": kind, "source": "hermes" });
            if let Some(t) = title {
                p["title"] = serde_json::json!(t);
            }
            if let Some(b) = body {
                p["body"] = serde_json::json!(b);
            }
            ("ui.open", p, 0)
        }
        Cmd::UiClose { id } => ("ui.close", serde_json::json!({ "id": id }), 0),
        Cmd::UiAction {
            window_id,
            action,
            ssid,
            password,
        } => {
            let mut values = serde_json::Map::new();
            if let Some(s) = ssid {
                values.insert("ssid".into(), serde_json::json!(s));
            }
            if let Some(pw) = password {
                values.insert("password".into(), serde_json::json!(pw));
            }
            (
                "ui.action",
                serde_json::json!({
                    "window_id": window_id,
                    "action": action,
                    "values": values,
                }),
                1 << 6, // HITL
            )
        }
        Cmd::OrbGet => ("orb.get", serde_json::json!({}), 0),
        Cmd::OrbSet { state, pattern } => {
            let mut p = serde_json::json!({ "presets": true });
            if let Some(s) = state {
                p["state"] = serde_json::json!(s);
            }
            if let Some(pat) = pattern {
                p["pattern"] = serde_json::json!(pat);
            }
            ("orb.set", p, 0)
        }
        Cmd::NixPropose { nix } => (
            "nix.propose",
            serde_json::json!({ "nix": nix }),
            1 << 5, // NIX_WRITE
        ),
        Cmd::NixApply { confirm } => (
            "nix.apply",
            serde_json::json!({ "hitl_confirmed": confirm }),
            1 << 5,
        ),
        Cmd::SecurityRotate => (
            "security.rotate",
            serde_json::json!({}),
            (1 << 5) | (1 << 6),
        ),
        Cmd::WifiScan => ("wifi.scan", serde_json::json!({}), 0),
        Cmd::WifiConnect {
            ssid,
            password,
            confirm,
        } => (
            "wifi.connect",
            serde_json::json!({
                "ssid": ssid,
                "password": password,
                "hitl_confirmed": confirm,
            }),
            1 << 6, // HITL
        ),
        Cmd::VoiceStatus => ("voice.status", serde_json::json!({}), 0),
        Cmd::VoiceTurn {
            prompt,
            record_secs,
        } => (
            "voice.turn",
            serde_json::json!({ "prompt": prompt, "record_secs": record_secs, "play": true }),
            1 | 16, // INFER | MEMORY
        ),
        Cmd::TransmuteStatus => ("transmute.status", serde_json::json!({}), 0),
        Cmd::Transmute {
            no_download,
            no_hotswap,
            locale,
            confirm,
        } => (
            "transmute.run",
            serde_json::json!({
                "download_models": !no_download,
                "speak_greeting": true,
                "hotswap_now": !no_hotswap,
                "locale_override": locale,
                "hitl_confirmed": confirm,
            }),
            (1 << 1) | (1 << 2) | (1 << 5), // NET | FS | NIX_WRITE
        ),
    };
    // Token-first: se o ficheiro existir, a CLI fica LocalAdmin também no Unix.
    // Sem token, o daemon usa SO_PEERCRED (Jarbas/LocalUser).
    let token = load_token(args.token.clone(), args.token_file.clone());
    if args.tcp.is_some() && token.is_none() {
        eprintln!(
            "nosctl: TCP requires --token/--token-file (or NOSD_IPC_TOKEN/NOSD_IPC_TOKEN_FILE)"
        );
        std::process::exit(2);
    }

    let req = Request {
        id: 1,
        method: method.into(),
        caps, // ignored by daemon (I.1); kept for wire compat
        cap_gen: 1,
        agent: "nosctl".into(), // legacy hint; never auth
        token,
        params,
    };
    let enc = encode(&req)?;

    let resp = if let Some(addr) = args.tcp {
        let mut s = tokio::net::TcpStream::connect(addr).await?;
        s.write_all(&enc).await?;
        read_resp(&mut s).await?
    } else {
        #[cfg(unix)]
        {
            let mut s = tokio::net::UnixStream::connect(&args.socket).await?;
            s.write_all(&enc).await?;
            read_resp(&mut s).await?
        }
        #[cfg(not(unix))]
        {
            let mut s = tokio::net::TcpStream::connect("127.0.0.1:7700").await?;
            s.write_all(&enc).await?;
            read_resp(&mut s).await?
        }
    };
    println!("{}", serde_json::to_string_pretty(&resp)?);
    if !resp.ok {
        std::process::exit(1);
    }
    Ok(())
}

fn load_token(explicit: Option<String>, file: Option<PathBuf>) -> Option<String> {
    if let Some(token) = explicit {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(path) = file {
        paths.push(path);
    }
    if let Ok(path) = std::env::var("NOSD_IPC_TOKEN_FILE") {
        paths.push(PathBuf::from(path));
    }
    paths.push(PathBuf::from("/run/nosd/ipc.token"));
    paths.push(PathBuf::from("/var/lib/nosd/ipc.token"));
    paths.push(PathBuf::from("data/ipc.token"));
    for path in paths {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let token = text.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

async fn read_resp<S: AsyncReadExt + Unpin>(s: &mut S) -> anyhow::Result<Response> {
    let mut acc = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = s.read(&mut buf).await?;
        if n == 0 {
            anyhow::bail!("eof");
        }
        acc.extend_from_slice(&buf[..n]);
        if let Some((body, _)) = take_frame(&acc)? {
            return Ok(decode_body(&body)?);
        }
    }
}
