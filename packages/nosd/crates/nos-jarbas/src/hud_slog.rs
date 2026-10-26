//! Fallback HUD — slog no journal (sem pixels). Sempre disponível.

use crate::rpc::{next_req, Endpoint};
use std::time::Duration;
use tracing::{info, warn};

pub async fn run(endpoint: Endpoint, interval_secs: u64) -> anyhow::Result<()> {
    info!(target: "jarbas", "HUD slog loop interval={}s (pixels = feature slint-ui)", interval_secs);
    let mut id = 1u64;
    let mut fail_streak = 0u32;
    loop {
        match endpoint
            .rpc(next_req(&mut id, "health", 0, serde_json::json!({})))
            .await
        {
            Ok(r) if r.ok => {
                fail_streak = 0;
                info!(target: "jarbas", "hud health={}", r.result);
            }
            Ok(r) => {
                fail_streak = fail_streak.saturating_add(1);
                warn!(target: "jarbas", "health not ok {:?}", r.error);
            }
            Err(e) => {
                fail_streak = fail_streak.saturating_add(1);
                warn!(target: "jarbas", "nosd unreachable ({e}) streak={fail_streak}");
            }
        }

        if let Ok(r) = endpoint
            .rpc(next_req(&mut id, "score", 0, serde_json::json!({})))
            .await
        {
            if r.ok {
                info!(target: "jarbas", "hud score={}", r.result);
            }
        }

        if let Ok(r) = endpoint
            .rpc(next_req(
                &mut id,
                "cognitive.tick",
                0,
                serde_json::json!({}),
            ))
            .await
        {
            if r.ok {
                info!(target: "jarbas", "hud tick={}", r.result);
            }
        }

        let wait = if fail_streak > 3 {
            Duration::from_secs(interval_secs.saturating_mul(2).max(5))
        } else {
            Duration::from_secs(interval_secs.max(1))
        };
        tokio::time::sleep(wait).await;
    }
}
