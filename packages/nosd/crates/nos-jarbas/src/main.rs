//! JARBAS HUD client — slog journal (default) ou Slint pixels (`--ui` + feature `slint-ui`).
//! Cage kiosk: prefer `--ui` quando o bin foi built com `slint-ui`.

mod hud_slog;
mod rpc;

#[cfg(feature = "slint-ui")]
mod slint_app;

use clap::Parser;
use rpc::{Endpoint, next_req};
use tracing::{info, warn};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "/run/nosd/nosd.sock")]
    socket: String,
    #[arg(long)]
    tcp: Option<String>,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();
    let endpoint = Endpoint::from_parts(&args.socket, args.tcp.as_deref());

    if args.once {
        let mut id = 1u64;
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
        } else {
            warn!(target: "jarbas", "err={:?}", resp.error);
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.ui {
        #[cfg(feature = "slint-ui")]
        {
            // Slint owns the event loop — leave tokio runtime
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

    hud_slog::run(endpoint, args.interval_secs).await
}
