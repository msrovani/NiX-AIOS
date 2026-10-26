//! HW.2 / U.2 — SelfHeal recipe stub CapGate Deny-by-default (0.5.0).
//! Cards ricos Slint / firmware Nix pleno = PARTIAL residual.

use nos_core::{CapBits, CapGate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfHealRecipe {
    pub id: String,
    pub action: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfHealDecision {
    pub allow: bool,
    pub reason: String,
}

/// Deny-by-default: exige CapGate NIX_WRITE + HITL confirm flag.
pub fn evaluate_self_heal(
    gate: &CapGate,
    recipe: &SelfHealRecipe,
    hitl_confirmed: bool,
) -> SelfHealDecision {
    if !hitl_confirmed {
        return SelfHealDecision {
            allow: false,
            reason: "HITL confirm required (deny-by-default)".into(),
        };
    }
    if gate.allow(CapBits::NIX_WRITE, gate.generation).is_err() {
        return SelfHealDecision {
            allow: false,
            reason: format!(
                "CapGate missing NIX_WRITE for recipe {}/{}",
                recipe.id, recipe.action
            ),
        };
    }
    SelfHealDecision {
        allow: true,
        reason: format!("allowed stub {} → {}", recipe.action, recipe.target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_without_hitl() {
        let gate = CapGate::new(CapBits::NIX_WRITE, 1);
        let r = SelfHealRecipe {
            id: "fw1".into(),
            action: "nix_fragment".into(),
            target: "wifi".into(),
        };
        let d = evaluate_self_heal(&gate, &r, false);
        assert!(!d.allow);
    }

    #[test]
    fn deny_without_cap() {
        let gate = CapGate::new(CapBits::NONE, 1);
        let r = SelfHealRecipe {
            id: "fw1".into(),
            action: "nix_fragment".into(),
            target: "wifi".into(),
        };
        let d = evaluate_self_heal(&gate, &r, true);
        assert!(!d.allow);
    }

    #[test]
    fn allow_with_cap_and_hitl() {
        let gate = CapGate::new(CapBits::NIX_WRITE, 1);
        let r = SelfHealRecipe {
            id: "fw1".into(),
            action: "nix_fragment".into(),
            target: "wifi".into(),
        };
        let d = evaluate_self_heal(&gate, &r, true);
        assert!(d.allow);
    }
}
