//! CapGate bitmask — server-authored (ADR-0118 / remigração I.1).
//!
//! A identidade NÃO vem do payload. O transporte resolve o perfil:
//! - Unix: `SO_PEERCRED` (uid/gid) no `nosd`
//! - TCP/HTTP: token de processo, rotacionável
//!
//! `req.caps` e `req.agent` são legado e nunca elevam permissão.

use serde::{Deserialize, Serialize};

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
    /// HITL / janelas JARBAS (abrir Escalate, submeter ação).
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

/// Perfis server-authored. A resolução é feita pelo transporte, não pelo cliente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerProfile {
    /// Root ou membro do grupo admin (CLI/operador).
    LocalAdmin,
    /// Utilizador local comum (chat/leitura; sem HITL/NIX_WRITE/spawn).
    LocalUser,
    /// HUD JARBAS / cage (uid do daemon).
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
    /// Mapeia credenciais Unix (`SO_PEERCRED`) para perfil.
    ///
    /// `uid == 0` → admin; `uid == daemon_uid` → kiosk JARBAS; `gid == admin_gid`
    /// (quando `admin_gid != 0`) → admin. O resto é utilizador local.
    pub fn from_unix_cred(uid: u32, gid: u32, daemon_uid: u32, admin_gid: u32) -> Self {
        if uid == 0 {
            return Self::LocalAdmin;
        }
        if uid == daemon_uid {
            return Self::Jarbas;
        }
        if admin_gid != 0 && gid == admin_gid {
            return Self::LocalAdmin;
        }
        Self::LocalUser
    }

    pub fn caps(self) -> CapBits {
        match self {
            Self::LocalAdmin => CapBits(CapBits::LEGIT_MASK),
            Self::LocalUser => CapBits(CapBits::INFER.0 | CapBits::MEMORY.0 | CapBits::OBSERVE.0),
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

/// Registo de perfis server-authored. Não há overrides por `agent` string.
#[derive(Debug, Default)]
pub struct CapRegistry;

impl CapRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn gate_for(&self, profile: PeerProfile, generation: u64) -> CapGate {
        CapGate::new(profile.caps(), generation)
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
    fn profiles_are_server_authored() {
        let jarbas = PeerProfile::Jarbas.caps();
        assert!(jarbas.contains(CapBits::HITL));
        assert!(jarbas.contains(CapBits::INFER));
        assert!(!jarbas.contains(CapBits::NIX_WRITE));
        assert!(!jarbas.contains(CapBits::AGENT_SPAWN));

        let user = PeerProfile::LocalUser.caps();
        assert!(user.contains(CapBits::INFER));
        assert!(user.contains(CapBits::MEMORY));
        assert!(!user.contains(CapBits::HITL));
        assert!(!user.contains(CapBits::NIX_WRITE));
        assert!(!user.contains(CapBits::AGENT_SPAWN));

        let admin = PeerProfile::LocalAdmin.caps();
        assert!(admin.contains(CapBits::NIX_WRITE));
        assert!(admin.contains(CapBits::HITL));
        assert!(admin.contains(CapBits::AGENT_SPAWN));
    }

    #[test]
    fn unix_cred_mapping() {
        assert_eq!(
            PeerProfile::from_unix_cred(0, 0, 1000, 1000),
            PeerProfile::LocalAdmin
        );
        assert_eq!(
            PeerProfile::from_unix_cred(1000, 1000, 1000, 1000),
            PeerProfile::Jarbas
        );
        assert_eq!(
            PeerProfile::from_unix_cred(1001, 1000, 1000, 1000),
            PeerProfile::LocalAdmin
        );
        assert_eq!(
            PeerProfile::from_unix_cred(1001, 1001, 1000, 1000),
            PeerProfile::LocalUser
        );
    }

    #[test]
    fn unknown_has_zero_caps() {
        assert_eq!(PeerProfile::Unknown.caps(), CapBits::NONE);
    }

    #[test]
    fn registry_gate_uses_profile_not_agent() {
        let reg = CapRegistry::new();
        let jarbas = reg.gate_for(PeerProfile::Jarbas, 1);
        assert!(jarbas.allow(CapBits::INFER, 1).is_ok());
        assert!(jarbas.allow(CapBits::NIX_WRITE, 1).is_err());

        let admin = reg.gate_for(PeerProfile::LocalAdmin, 1);
        assert!(admin.allow(CapBits::NIX_WRITE, 1).is_ok());
    }
}
