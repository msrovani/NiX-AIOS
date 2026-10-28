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
    /// Requer deliberação Cortex/Hermes + grant (`--confirm-id`) para download.
    Fetch {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        file: Option<String>,
        #[arg(long)]
        filename: Option<String>,
        /// Grant server-authored de `ui.action` (kind=blob.fetch).
        /// Obtenha com: nosctl consent blob.fetch
        #[arg(long)]
        confirm_id: Option<String>,
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
        /// S-04: tipo de grant que `ui.action` pode emitir (ex. blob.fetch).
        #[arg(long)]
        grant_kind: Option<String>,
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
    /// M9: instala uma skill wasm (ficheiro .wasm) e activa-a.
    #[command(name = "skill-install")]
    SkillInstall {
        name: String,
        path: String,
        /// S2: agrupamento para escolha (default = nome).
        #[arg(long)]
        focus: Option<String>,
        #[arg(long)]
        version: Option<String>,
        /// S2: publisher (hex) e assinatura (base64) do manifesto.
        #[arg(long)]
        publisher: Option<String>,
        #[arg(long)]
        sig: Option<String>,
    },
    /// S2: assina um manifesto com a chave do no (imprime publisher+sig).
    #[command(name = "skill-sign")]
    SkillSign {
        name: String,
        #[arg(long, default_value = "")]
        focus: String,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        /// Hash BLAKE3 do .wasm (o que `skill-install` imprime).
        hash: String,
    },
    /// S2c: revoga um publisher (hex) - a assinatura deixa de valer.
    #[command(name = "skill-revoke")]
    SkillRevoke {
        publisher: String,
    },
    /// M9.1: empurra a skill activa para um peer (verifica hash no destino).
    #[command(name = "skill-push")]
    SkillPush {
        addr: String,
        name: String,
    },
    /// M9: lista skills activas.
    #[command(name = "skill-list")]
    SkillList,
    /// M9: corre a skill (export `run`, i32 -> i32).
    #[command(name = "skill-run")]
    SkillRun {
        name: String,
        #[arg(long, default_value_t = 0)]
        arg: i32,
    },
    /// M8: escreve uma chave no CRDT local.
    #[command(name = "crdt-put")]
    CrdtPut {
        key: String,
        value: String,
        #[arg(long, default_value_t = 0)]
        tick: u64,
    },
    /// M8: lê uma chave do CRDT local.
    #[command(name = "crdt-get")]
    CrdtGet {
        key: String,
    },
    /// M8: anti-entropy com um peer (estado completo, link assinado).
    #[command(name = "crdt-sync")]
    CrdtSync {
        addr: String,
    },
    /// M7: envia um job para um peer (link TCP autenticado, resultado assinado).
    #[command(name = "job-submit")]
    JobSubmit {
        /// Endereço do worker: `IP:PORT` (ex. 192.168.50.2:19001).
        addr: String,
        prompt: String,
        #[arg(long, default_value_t = 30000)]
        deadline_ms: u64,
        #[arg(long, default_value_t = 256)]
        budget_tokens: u32,
    },
    /// S-04: pede um grant HITL em texto (abre a janela + submete).
    /// Imprime {"result":{"confirm_id":"..."}} para usar em --confirm-id.
    Consent {
        /// Ex.: transmute, blob.fetch, nix.apply, wifi.connect, self.heal.
        kind: String,
        #[arg(long)]
        title: Option<String>,
    },
    /// Arma apply root (nosd-nix-apply.service). Exige grant (--confirm-id).
    NixApply {
        /// Grant server-authored (kind=nix.apply). Ver `nosctl consent nix.apply`.
        #[arg(long)]
        confirm_id: Option<String>,
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
        /// Grant server-authored (kind=wifi.connect). Ver `nosctl consent wifi.connect`.
        #[arg(long)]
        confirm_id: Option<String>,
    },
    /// Estado STT (Whisper) + TTS (Piper/Espeak) + idioma detectado.
    #[command(name = "voice-status")]
    VoiceStatus,
    /// STT de um WAV (ou gravação) — 0.6 dev/QEMU.
    #[command(name = "voice-stt")]
    VoiceStt {
        #[arg(long)]
        wav_path: Option<String>,
        #[arg(long, default_value_t = 4)]
        record_secs: u32,
    },
    /// TTS para WAV; `--no-play` não tenta tocar áudio.
    #[command(name = "voice-tts")]
    VoiceTts {
        text: String,
        #[arg(long)]
        no_play: bool,
    },
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
        /// Grant server-authored (kind=transmute). Ver `nosctl consent transmute`.
        #[arg(long)]
        confirm_id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // S2: assinatura local do manifesto (nao fala com o daemon).
    if let Cmd::SkillSign {
        name,
        focus,
        version,
        hash,
    } = &args.cmd
    {
        let key_path = std::env::var("NOSD_MESH_KEY")
            .unwrap_or_else(|_| "/var/lib/nosd/keys/mesh.key".to_string());
        let seed = std::fs::read(&key_path)
            .map_err(|e| anyhow::anyhow!("chave {key_path}: {e} (precisa de sudo?)"))?;
        if seed.len() != 32 {
            anyhow::bail!("chave {key_path} tem de ter 32 bytes");
        }
        let mut s = [0u8; 32];
        s.copy_from_slice(&seed);
        let id = nos_mesh::Identity::from_seed(s, Default::default(), true);
        let focus = if focus.is_empty() { name } else { focus };
        let msg = nosd_lib::rpc::skills::publisher_msg(name, focus, version, hash);
        let sig = id.sign(msg.as_bytes());
        use base64::Engine as _;
        let publisher: String = id.public_key().iter().map(|b| format!("{b:02x}")).collect();
        let out = serde_json::json!({
            "publisher": publisher,
            "sig": base64::engine::general_purpose::STANDARD.encode(sig),
            "msg": msg,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let (method, params, caps) = match &args.cmd {
        Cmd::SkillInstall {
            name,
            path,
            focus,
            version,
            publisher,
            sig,
        } => {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("skill-install: {e}");
                    std::process::exit(2);
                }
            };
            let mut p = serde_json::json!({ "name": name });
            if path.ends_with(".wat") {
                p["wat"] = serde_json::json!(String::from_utf8_lossy(&bytes));
            } else {
                use base64::Engine as _;
                p["wasm_b64"] =
                    serde_json::json!(base64::engine::general_purpose::STANDARD.encode(&bytes));
            }
            if let Some(f) = focus {
                p["focus"] = serde_json::json!(f);
            }
            if let Some(v) = version {
                p["version"] = serde_json::json!(v);
            }
            if let Some(x) = publisher {
                p["publisher"] = serde_json::json!(x);
            }
            if let Some(x) = sig {
                p["sig"] = serde_json::json!(x);
            }
            ("skill.install", p, 1 << 2) // FS
        }
        Cmd::SkillRevoke { publisher } => (
            "crdt.put",
            serde_json::json!({
                "key": format!("skill/revoked/{publisher}"),
                "value": "1",
            }),
            1 << 4, // MEMORY
        ),
        Cmd::SkillList => ("skill.list", serde_json::json!({}), 0),
        Cmd::SkillRun { name, arg } => (
            "skill.run",
            serde_json::json!({ "name": name, "arg": arg }),
            0,
        ),
        Cmd::SkillPush { addr, name } => (
            "skill.push",
            serde_json::json!({ "addr": addr, "name": name }),
            1 << 2, // FS
        ),
        Cmd::CrdtPut { key, value, tick } => (
            "crdt.put",
            serde_json::json!({ "key": key, "value": value, "tick": tick }),
            1 << 4,
        ),
        Cmd::CrdtGet { key } => ("crdt.get", serde_json::json!({ "key": key }), 0),
        Cmd::CrdtSync { addr } => ("crdt.sync", serde_json::json!({ "addr": addr }), 1 << 4),
        Cmd::JobSubmit {
            addr,
            prompt,
            deadline_ms,
            budget_tokens,
        } => (
            "job.submit",
            serde_json::json!({
                "addr": addr,
                "prompt": prompt,
                "deadline_ms": deadline_ms,
                "budget_tokens": budget_tokens,
            }),
            1, // INFER
        ),
        // Consent e SkillSign sao tratados antes deste match.
        Cmd::Consent { .. } => ("consent.noop", serde_json::json!({}), 0),
        Cmd::SkillSign { .. } => ("skill-sign.noop", serde_json::json!({}), 0),
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
            confirm_id,
            estimated_mb,
        } => {
            let mut p = serde_json::json!({ "estimated_mb": estimated_mb });
            if let Some(id) = confirm_id {
                p["confirm_id"] = serde_json::json!(id);
            }
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
        Cmd::UiOpen {
            kind,
            title,
            body,
            grant_kind,
        } => {
            let mut p = serde_json::json!({ "kind": kind, "source": "hermes" });
            if let Some(t) = title {
                p["title"] = serde_json::json!(t);
            }
            if let Some(b) = body {
                p["body"] = serde_json::json!(b);
            }
            if let Some(g) = grant_kind {
                p["grant_kind"] = serde_json::json!(g);
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
        Cmd::NixApply { confirm_id } => (
            "nix.apply",
            serde_json::json!({ "confirm_id": confirm_id }),
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
            confirm_id,
        } => (
            "wifi.connect",
            serde_json::json!({
                "ssid": ssid,
                "password": password,
                "confirm_id": confirm_id,
            }),
            1 << 6, // HITL
        ),
        Cmd::VoiceStatus => ("voice.status", serde_json::json!({}), 0),
        Cmd::VoiceStt {
            wav_path,
            record_secs,
        } => (
            "voice.stt",
            serde_json::json!({ "wav_path": wav_path, "record_secs": record_secs }),
            1,
        ),
        Cmd::VoiceTts { text, no_play } => (
            "voice.tts",
            serde_json::json!({ "text": text, "play": !*no_play }),
            1,
        ),
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
            confirm_id,
        } => (
            "transmute.run",
            serde_json::json!({
                "download_models": !no_download,
                "speak_greeting": true,
                "hotswap_now": !no_hotswap,
                "locale_override": locale,
                "confirm_id": confirm_id,
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

    // S-04 (texto): abre a janela HITL no servidor e submete-a. O confirm_id
    // e emitido pelo SERVIDOR (kind-bound, single-use, TTL). O cliente nunca
    // manda um booleano de consentimento.
    if let Cmd::Consent { kind, title } = &args.cmd {
        let mut p = serde_json::json!({
            "kind": "hitl_confirm",
            "source": "hermes",
            "grant_kind": kind,
        });
        if let Some(t) = title {
            p["title"] = serde_json::json!(t);
        }
        let opened = rpc_call(&args, token.clone(), "ui.open", p, 0).await?;
        if !opened.ok {
            print_json(&opened)?;
            std::process::exit(1);
        }
        let wid = opened
            .result
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let acted = rpc_call(
            &args,
            token.clone(),
            "ui.action",
            serde_json::json!({ "window_id": wid, "action": "submit", "values": {} }),
            1 << 6,
        )
        .await?;
        print_json(&acted)?;
        if !acted.ok {
            std::process::exit(1);
        }
        return Ok(());
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
    print_json(&resp)?;
    if !resp.ok {
        std::process::exit(1);
    }
    Ok(())
}

/// Escreve a resposta JSON. `BrokenPipe` (grep -q/head/less) nao e erro:
/// sem isto o `println!` entra em panico e o comando sai com 101.
fn print_json(v: &impl serde::Serialize) -> anyhow::Result<()> {
    use std::io::Write;
    let s = serde_json::to_string_pretty(v)?;
    let mut out = std::io::stdout().lock();
    let r = out
        .write_all(s.as_bytes())
        .and_then(|_| out.write_all(b"\n"));
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(e.into()),
    }
}

async fn rpc_call(
    args: &Args,
    token: Option<String>,
    method: &str,
    params: serde_json::Value,
    caps: u32,
) -> anyhow::Result<Response> {
    let req = Request {
        id: 1,
        method: method.into(),
        caps,
        cap_gen: 1,
        agent: "nosctl".into(),
        token,
        params,
    };
    let enc = encode(&req)?;
    if let Some(addr) = &args.tcp {
        let mut s = tokio::net::TcpStream::connect(addr).await?;
        s.write_all(&enc).await?;
        return read_resp(&mut s).await;
    }
    #[cfg(unix)]
    {
        let mut s = tokio::net::UnixStream::connect(&args.socket).await?;
        s.write_all(&enc).await?;
        read_resp(&mut s).await
    }
    #[cfg(not(unix))]
    {
        let mut s = tokio::net::TcpStream::connect("127.0.0.1:7700").await?;
        s.write_all(&enc).await?;
        read_resp(&mut s).await
    }
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
