//! JARBAS HUD client — slog journal (default) ou Slint pixels (`--ui` + feature `slint-ui`).
//! Cage kiosk: prefer `--ui` quando o bin foi built com `slint-ui`.

mod hud_slog;
mod rpc;

#[cfg(feature = "slint-ui")]
mod slint_app;

use clap::Parser;
use rpc::{next_req, Endpoint};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Parser, Debug)]
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
    /// One-shot chat then exit (lab).
    #[arg(long)]
    once: bool,
    #[arg(long, default_value = "oi")]
    chat: String,
    #[arg(long, default_value_t = 2)]
    interval_secs: u64,
    /// Abrir janela Slint (exige build `--features slint-ui`).
    #[arg(long)]
    ui: bool,
}

/// Runtime próprio por chamada async (NÃO `#[tokio::main]`).
///
/// O HUD Slint é o event loop e usa `rpc_blocking`; se o runtime tokio
/// estivesse "entrado" no thread principal, `block_on` aninhado fazia panic.
fn tokio_rt() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();
    let token = if args.tcp.is_some() {
        rpc::load_token(args.token.clone(), args.token_file.clone())
    } else {
        None
    };
    if args.tcp.is_some() && token.is_none() {
        anyhow::bail!(
            "--tcp requires --token/--token-file (or NOSD_IPC_TOKEN/NOSD_IPC_TOKEN_FILE)"
        );
    }
    let endpoint = Endpoint::from_parts(&args.socket, args.tcp.as_deref(), token);

    if args.once {
        let mut id = 1u64;
        return tokio_rt()?.block_on(async move {
            let resp = endpoint
                .rpc(next_req(
                    &mut id,
                    "chat",
                    1 | 16,
                    serde_json::json!({ "prompt": args.chat }),
                ))
                .await?;
            if resp.ok {
                info!(target: "jarbas", "reply={}", resp.result);
                Ok(())
            } else {
                warn!(target: "jarbas", "err={:?}", resp.error);
                Err(anyhow::anyhow!("chat failed"))
            }
        });
    }

    if args.ui {
        #[cfg(feature = "slint-ui")]
        {
            // Slint owns the event loop — sem runtime tokio activo neste thread.
            return slint_app::run(endpoint, args.interval_secs);
        }
        #[cfg(not(feature = "slint-ui"))]
        {
            warn!(
                target: "jarbas",
                "--ui requested but binary built without feature slint-ui — falling back to slog HUD"
            );
        }
    }

    tokio_rt()?.block_on(hud_slog::run(endpoint, args.interval_secs))
}
