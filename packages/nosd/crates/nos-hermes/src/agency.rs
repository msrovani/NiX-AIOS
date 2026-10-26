//! Frota Agency EventDriven — AGENT.md data-driven (A.1); anti-stub em massa.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleKind {
    Oneshot,
    Continuous,
    EventDriven,
    PollEvery { every: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub name: String,
    pub schedule: ScheduleKind,
    pub goal: String,
    pub signed: bool,
}

#[derive(Debug, Default)]
pub struct AgencyFleet {
    agents: HashMap<String, AgentManifest>,
}

impl AgencyFleet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Regista só se assinado ou seed nativo explícito — unsigned stub = deny.
    pub fn register(&mut self, m: AgentManifest) -> Result<(), &'static str> {
        if !m.signed && !m.name.starts_with("native.") {
            return Err("unsigned agent denied (anti-stub)");
        }
        if m.goal.trim().is_empty() || m.goal == "TODO" {
            return Err("empty goal denied");
        }
        self.agents.insert(m.name.clone(), m);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn list(&self) -> Vec<&AgentManifest> {
        self.agents.values().collect()
    }

    pub fn seed_natives(&mut self) {
        let seeds = [
            ("native.hermes", ScheduleKind::Continuous, "orchestrate OPAR"),
            ("native.observe", ScheduleKind::EventDriven, "Linux observe bus"),
            (
                "native.autolearn",
                ScheduleKind::PollEvery { every: 200 },
                "Seek+learn niches",
            ),
            (
                "native.sleep",
                ScheduleKind::PollEvery { every: 1000 },
                "SleepCycle consolidate",
            ),
        ];
        for (n, s, g) in seeds {
            let _ = self.register(AgentManifest {
                name: n.into(),
                schedule: s,
                goal: g.into(),
                signed: true,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_unsigned_stub() {
        let mut f = AgencyFleet::new();
        assert!(f
            .register(AgentManifest {
                name: "fake_agent_001".into(),
                schedule: ScheduleKind::EventDriven,
                goal: "TODO".into(),
                signed: false,
            })
            .is_err());
    }

    #[test]
    fn seed_natives_ok() {
        let mut f = AgencyFleet::new();
        f.seed_natives();
        assert!(f.len() >= 4);
    }
}
