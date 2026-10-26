//! Contrato IPC: métodos side-effect / caps conhecidos vs handlers presentes no dispatch.

use nos_core::CapBits;
use nos_hermes::SIDE_EFFECT_METHODS;
use nos_ipc::Request;

fn sample_req(method: &str) -> Request {
    Request {
        id: 1,
        method: method.into(),
        caps: 0,
        cap_gen: 1,
        agent: "nosctl".into(),
        token: None,
        params: serde_json::json!({}),
    }
}

#[test]
fn side_effect_methods_have_non_deny_caps_or_known() {
    for m in SIDE_EFFECT_METHODS {
        let req = sample_req(m);
        let cap = req.required_cap();
        assert_ne!(
            cap,
            CapBits::DENY_UNKNOWN,
            "side-effect method {m} must declare required_cap (got DENY_UNKNOWN)"
        );
    }
}

#[test]
fn self_heal_requires_nix_write_and_hitl() {
    let req = sample_req("self.heal.evaluate");
    let c = req.required_cap();
    assert!(c.contains(CapBits::NIX_WRITE));
    assert!(c.contains(CapBits::HITL));
}

#[test]
fn security_rotate_requires_admin_caps() {
    let req = sample_req("security.rotate");
    let c = req.required_cap();
    assert!(c.contains(CapBits::NIX_WRITE));
    assert!(c.contains(CapBits::HITL));
}

#[test]
fn mesh_skillsync_removed_from_side_effects() {
    assert!(!SIDE_EFFECT_METHODS.contains(&"mesh.skillsync"));
}

#[test]
fn fleet_health_and_recall_caps() {
    assert_eq!(sample_req("fleet.health").required_cap(), CapBits::NONE);
    assert_eq!(sample_req("memory.recall").required_cap(), CapBits::MEMORY);
    assert_eq!(
        sample_req("store.index").required_cap(),
        CapBits::AGENT_SPAWN
    );
}

#[test]
fn fnv_core_matches_mesh_factory() {
    let payload = b"hello mesh";
    assert_eq!(nos_core::fnv1a64(payload), nos_mesh::fnv1a64(payload));
    assert_eq!(nos_core::fnv1a64(payload), nos_factory::fnv1a64(payload));
}
