//! DB.2 — OPAR universal gate: side-effect IPC methods must persist Observe→Plan→Act→Verify.

use crate::{CognitiveAct, HermesEngine};

/// Métodos RPC com efeitos colaterais (manifesto remigração DB.2).
pub const SIDE_EFFECT_METHODS: &[&str] = &[
    "chat",
    "chat.submit",
    "voice.turn",
    "nix.propose",
    "nix.apply",
    "nix.confirm",
    "nix.rollback",
    "nix.build_vm",
    "wifi.connect",
    "model.fetch",
    "firmware.fetch",
    "transmute.run",
    "agent.spawn",
    "factory.promote",
    "factory.synthesize",
    "factory.seek",
    "store.install",
    "systemctl.scoped",
    "audio.volume",
    "gpu.plan",
    "device.recipe",
    "mesh.skillsync",
];

pub fn is_side_effect(method: &str) -> bool {
    SIDE_EFFECT_METHODS.contains(&method)
}

impl HermesEngine {
    /// Central helper — dispatch chama após sucesso quando o handler não gravou OPAR inline.
    pub fn opar_side_effect(
        &self,
        method: &str,
        observe: &str,
        plan: &str,
        act: &str,
        verify: &str,
    ) -> CognitiveAct {
        self.record_opar(method, observe, plan, act, verify)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_effect_inventory_covers_chat_and_spawn() {
        assert!(is_side_effect("chat"));
        assert!(is_side_effect("agent.spawn"));
        assert!(is_side_effect("store.install"));
        assert!(is_side_effect("nix.build_vm"));
        assert!(is_side_effect("systemctl.scoped"));
        assert!(!is_side_effect("health"));
        assert!(!is_side_effect("route.explain"));
        assert!(!is_side_effect("observe.snapshot"));
    }
}
