//! U.2 — Trust (token, agent, skill) + SafetyInvariant stubs (não é port k_ai completo).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustToken {
    pub token: u64,
    pub agent: String,
    pub skill: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyInvariant {
    /// Heap acima do teto → fail-closed (stub: limiar fixo MB).
    HeapHeadroom { max_used_mb: u64 },
    /// CapGate exige bit presente.
    CapHeld { bit: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolation {
    pub invariant: String,
    pub agent: String,
    pub skill: String,
    pub detail: String,
}

static VIOLATION_COUNT: AtomicU64 = AtomicU64::new(0);

/// Trust triplet — token deve bater + agent/skill coerentes com o pedido.
pub fn trust_allow(token: &TrustToken, required: u64, agent: &str, skill: &str) -> bool {
    token.token == required && token.agent == agent && token.skill == skill
}

pub fn check_invariant(inv: SafetyInvariant, heap_used_mb: u64, held_caps: u32) -> bool {
    match inv {
        SafetyInvariant::HeapHeadroom { max_used_mb } => heap_used_mb <= max_used_mb,
        SafetyInvariant::CapHeld { bit } => (held_caps & bit) == bit,
    }
}

pub fn note_violation(_v: SafetyViolation) -> u64 {
    VIOLATION_COUNT.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn violation_count() -> u64 {
    VIOLATION_COUNT.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_allow_triplet() {
        let t = TrustToken {
            token: 42,
            agent: "hermes".into(),
            skill: "chat".into(),
        };
        assert!(trust_allow(&t, 42, "hermes", "chat"));
        assert!(!trust_allow(&t, 43, "hermes", "chat"));
    }

    #[test]
    fn heap_invariant_stub() {
        assert!(check_invariant(
            SafetyInvariant::HeapHeadroom { max_used_mb: 2048 },
            512,
            0
        ));
        assert!(!check_invariant(
            SafetyInvariant::HeapHeadroom { max_used_mb: 100 },
            512,
            0
        ));
    }

    #[test]
    fn note_violation_increments() {
        let before = violation_count();
        let _ = note_violation(SafetyViolation {
            invariant: "cap".into(),
            agent: "nosctl".into(),
            skill: "infer".into(),
            detail: "denied".into(),
        });
        assert_eq!(violation_count(), before + 1);
    }
}
