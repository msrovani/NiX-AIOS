//! SoulMirror thin (M.3) — snapshot de frota/persona para EventBus + HUD slog.
//! Lab 2-nó e2e = AWAITING_HW; aqui só contrato + merge CRDT local.

use crate::crdt::CrdtStore;
use crate::fleet_health::{fleet_worst, MachineHealth};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoulMirrorSnapshot {
    pub node_id: u32,
    pub persona: String,
    pub mood: String,
    pub fleet_verdict: String,
    pub peer_count: u32,
    pub tick: u64,
    /// Linha curta para HUD / slog.
    pub slog_line: String,
}

impl SoulMirrorSnapshot {
    pub fn from_fleet(
        node_id: u32,
        persona: &str,
        mood: &str,
        machines: &[MachineHealth],
        tick: u64,
    ) -> Self {
        let verdict = fleet_worst(machines);
        let peer_count = machines.len().saturating_sub(1) as u32;
        let fleet_verdict = verdict.as_str().to_string();
        let slog_line = format!(
            "SOUL_MIRROR node={node_id} persona={persona} mood={mood} fleet={fleet_verdict} peers={peer_count} t={tick}"
        );
        Self {
            node_id,
            persona: persona.into(),
            mood: mood.into(),
            fleet_verdict,
            peer_count,
            tick,
            slog_line,
        }
    }

    /// Grava campos no CRDT local (chave `soul/{node_id}`).
    pub fn into_crdt(&self, store: &mut CrdtStore) {
        store.put(
            self.node_id,
            format!("soul/{}", self.node_id),
            serde_json::to_string(self).unwrap_or_else(|_| self.slog_line.clone()),
            self.tick,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet_health::HealthVerdict;

    #[test]
    fn snapshot_slog_and_crdt() {
        let machines = vec![
            MachineHealth {
                node_id: 1,
                sys: HealthVerdict::Go,
                audio: HealthVerdict::Go,
                hub: HealthVerdict::Go,
                reason: "local".into(),
            },
            MachineHealth {
                node_id: 2,
                sys: HealthVerdict::Unknown,
                audio: HealthVerdict::Go,
                hub: HealthVerdict::Go,
                reason: "peer".into(),
            },
        ];
        let snap = SoulMirrorSnapshot::from_fleet(1, "jarbas", "focus", &machines, 42);
        assert!(snap.slog_line.contains("SOUL_MIRROR"));
        assert!(snap.slog_line.contains("peers=1"));
        assert_eq!(snap.fleet_verdict, "unknown");
        let mut store = CrdtStore::new();
        snap.into_crdt(&mut store);
        assert!(store.get("soul/1").is_some());
    }
}
