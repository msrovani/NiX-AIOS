//! CapGate bitmask — server-authored (ADR-0118 / remigração I.1).
//! Cliente NÃO escolhe caps: o daemon resolve por `agent` (+ geração).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapBits(pub u32);

impl CapBits {
    pub const NONE: Self = Self(0);
    pub const INFER: Self = Self(1 << 0);
    pub const NET: Self = Self(1 << 1);
    pub const FS: Self = Self(1 << 2);
    pub const AGENT_SPAWN: Self = Self(1 << 3);
    pub const MEMORY: Self = Self(1 << 4);
    pub const NIX_WRITE: Self = Self(1 << 5);
    /// HITL / janelas JARBAS (abrir Escalate, submeter acção).
    pub const HITL: Self = Self(1 << 6);
    /// Observe-only (journal/udev/hwmon) — sem side-effects.
    pub const OBSERVE: Self = Self(1 << 7);
    /// Bits máximos legítimos (nunca ALL 0xFFFF_FFFF do cliente).
    pub const LEGIT_MASK: u32 = (1 << 8) - 1;
    /// Perfil interno Hermes (todos os bits legítimos).
    pub const FULL: Self = Self((1 << 8) - 1);
    /// Sentinel: método IPC desconhecido — só passa se held==ALL (ninguém tem).
    pub const DENY_UNKNOWN: Self = Self(0xFFFF_FFFF);

    pub fn contains(self, required: CapBits) -> bool {
        (self.0 & required.0) == required.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn is_client_all(bits: u32) -> bool {
        bits == 0xFFFF_FFFF || bits == u32::MAX
    }
}

#[derive(Debug, Clone)]
pub struct CapGate {
    pub held: CapBits,
    pub generation: u64,
}

impl CapGate {
    pub fn new(held: CapBits, generation: u64) -> Self {
        Self { held, generation }
    }

    pub fn allow(&self, required: CapBits, current_gen: u64) -> Result<(), &'static str> {
        if self.generation != current_gen {
            return Err("capability generation revoked");
        }
        if !self.held.contains(required) {
            return Err("capability denied (bitmask)");
        }
        Ok(())
    }
}

/// Perfis server-authored. `req.caps` do cliente é ignorado (I.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerProfile {
    /// CLI local / operador (nosctl).
    LocalAdmin,
    /// HUD JARBAS / cage.
    Jarbas,
    /// Loop Continuous Hermes interno (não IPC).
    HermesInternal,
    /// Skill WASM Frugal — caps grafted no spawn.
    WasmSkill,
    /// ObserveAgent / nos-observe.
    Observe,
    /// Desconhecido → zero caps.
    Unknown,
}

impl PeerProfile {
    pub fn from_agent(agent: &str) -> Self {
        match agent.trim().to_ascii_lowercase().as_str() {
            "" | "nosctl" | "local" | "admin" | "local.admin" => Self::LocalAdmin,
            "jarbas" | "nos-jarbas" | "cage" => Self::Jarbas,
            "hermes" | "hermes.internal" => Self::HermesInternal,
            "wasm" | "wasm.skill" | "skill" => Self::WasmSkill,
            "observe" | "nos-observe" => Self::Observe,
            _ => Self::Unknown,
        }
    }

    pub fn caps(self) -> CapBits {
        match self {
            Self::LocalAdmin => CapBits(
                CapBits::INFER.0
                    | CapBits::NET.0
                    | CapBits::FS.0
                    | CapBits::AGENT_SPAWN.0
                    | CapBits::MEMORY.0
                    | CapBits::NIX_WRITE.0
                    | CapBits::HITL.0
                    | CapBits::OBSERVE.0,
            ),
            Self::Jarbas => CapBits(
                CapBits::INFER.0
                    | CapBits::MEMORY.0
                    | CapBits::HITL.0
                    | CapBits::FS.0
                    | CapBits::OBSERVE.0
                    | CapBits::NET.0,
            ),
            Self::HermesInternal => CapBits(CapBits::LEGIT_MASK),
            Self::WasmSkill => CapBits::NONE,
            Self::Observe => CapBits::OBSERVE,
            Self::Unknown => CapBits::NONE,
        }
    }
}

/// Registo de overrides por agent string (tests / runtime grant).
#[derive(Debug, Default)]
pub struct CapRegistry {
    overrides: RwLock<HashMap<String, CapBits>>,
}

impl CapRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&self, agent: &str, caps: CapBits) {
        if let Ok(mut g) = self.overrides.write() {
            g.insert(agent.to_ascii_lowercase(), caps);
        }
    }

    pub fn revoke_override(&self, agent: &str) {
        if let Ok(mut g) = self.overrides.write() {
            g.remove(&agent.to_ascii_lowercase());
        }
    }

    /// Resolve caps server-authored. `client_claimed` is never trusted (I.1).
    pub fn resolve(&self, agent: &str, client_claimed: u32) -> Result<CapBits, &'static str> {
        // ALL / raw client bits never elevate — profile or override only.
        let _client_ignored = client_claimed;
        if let Ok(g) = self.overrides.read() {
            if let Some(c) = g.get(&agent.to_ascii_lowercase()) {
                return Ok(*c);
            }
        }
        let profile = PeerProfile::from_agent(agent);
        if profile == PeerProfile::Unknown && !agent.is_empty() {
            return Err("unknown agent — capability denied");
        }
        Ok(profile.caps())
    }

    pub fn gate_for(
        &self,
        agent: &str,
        client_claimed: u32,
        generation: u64,
    ) -> Result<CapGate, &'static str> {
        let held = self.resolve(agent, client_claimed)?;
        Ok(CapGate::new(held, generation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_without_bit() {
        let g = CapGate::new(CapBits::INFER, 1);
        assert!(g.allow(CapBits::NET, 1).is_err());
        assert!(g.allow(CapBits::INFER, 1).is_ok());
    }

    #[test]
    fn revoke_on_gen_bump() {
        let g = CapGate::new(PeerProfile::LocalAdmin.caps(), 1);
        assert!(g.allow(CapBits::INFER, 2).is_err());
    }

    #[test]
    fn hitl_bit_gated() {
        let g = CapGate::new(CapBits::INFER, 1);
        assert!(g.allow(CapBits::HITL, 1).is_err());
        let g = CapGate::new(CapBits::HITL, 1);
        assert!(g.allow(CapBits::HITL, 1).is_ok());
    }

    #[test]
    fn client_all_ignored_server_authored() {
        let reg = CapRegistry::new();
        let caps = reg.resolve("jarbas", 0xFFFF_FFFF).unwrap();
        assert!(!CapBits::is_client_all(caps.0));
        assert!(caps.contains(CapBits::HITL));
        assert!(!caps.contains(CapBits::NIX_WRITE));
    }

    #[test]
    fn unknown_agent_denied() {
        let reg = CapRegistry::new();
        assert!(reg.resolve("evil.bot", 0).is_err());
    }

    #[test]
    fn nosctl_gets_nix_write() {
        let reg = CapRegistry::new();
        let caps = reg.resolve("nosctl", 0).unwrap();
        assert!(caps.contains(CapBits::NIX_WRITE));
    }

    #[test]
    fn grant_override() {
        let reg = CapRegistry::new();
        reg.grant("wasm.skill", CapBits::INFER.union(CapBits::MEMORY));
        let caps = reg.resolve("wasm.skill", 0).unwrap();
        assert!(caps.contains(CapBits::INFER));
    }
}
