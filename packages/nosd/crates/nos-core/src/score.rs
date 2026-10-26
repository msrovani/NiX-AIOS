//! NOSD_SCORE — sev contract (ADAPT ADR-0092).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sev {
    Ok,
    Warn,
    Fail,
    Trace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NosScore {
    pub phase: String,
    pub sev: Sev,
    pub msg: String,
    pub hub_slots: u8,
    pub memory_ok: bool,
    pub mesh_peers: u32,
    /// Worst-of fleet quando mesh activo (`fleet.health` parity).
    pub fleet_verdict: Option<String>,
    /// Linha curta para HUD (`n/a` explícito — nunca inventar 0).
    #[serde(default)]
    pub summary: String,
    /// S1 efectivo (keyword | agentjev | scorer_absent | …).
    #[serde(default)]
    pub s1_backend: String,
    /// `absent` | `url_set` | `cpu-echo` | nome do hotswap.
    #[serde(default)]
    pub llama_server: String,
    /// Landlock status honesty.
    #[serde(default)]
    pub landlock: String,
    /// Snapshot early boot se `/var/lib/nosd/boot-score.json` existir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_boot: Option<serde_json::Value>,
}

impl NosScore {
    pub fn line(&self) -> String {
        format!(
            "NOSD_SCORE phase={} sev={:?} hub={} mem={} peers={} s1={} llama={} ll={} | {}",
            self.phase,
            self.sev,
            self.hub_slots,
            self.memory_ok as u8,
            self.mesh_peers,
            if self.s1_backend.is_empty() {
                "n/a"
            } else {
                self.s1_backend.as_str()
            },
            if self.llama_server.is_empty() {
                "n/a"
            } else {
                self.llama_server.as_str()
            },
            if self.landlock.is_empty() {
                "n/a"
            } else {
                self.landlock.as_str()
            },
            if let Some(v) = &self.fleet_verdict {
                format!("fleet={v} | {}", self.msg)
            } else {
                self.msg.clone()
            }
        )
    }

    /// Preenche `summary` a partir de `line()` (HUD).
    pub fn with_summary(mut self) -> Self {
        self.summary = self.line();
        self
    }
}
