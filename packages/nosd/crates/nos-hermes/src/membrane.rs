//! Membrane CapGate + MCP allowlist (P.6 / Wetware pattern) — zero ambient authority.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MembraneGrant {
    pub caps_bits: u32,
    pub fs_allow: Vec<String>,
    pub net_allow: Vec<String>,
    pub mcp_servers: Vec<String>,
}

/// Graft caps onto WASM skill — never inherit ambient process caps.
pub fn graft(base_none: u32, grant: &MembraneGrant) -> u32 {
    let _ = base_none;
    grant.caps_bits
}

pub fn mcp_allowed(grant: &MembraneGrant, server: &str) -> bool {
    grant.mcp_servers.iter().any(|s| s == server || s == "*")
}

/// Stub P.6 — peers Cross-OS conhecidos (patterns, não hosts reais).
pub fn discover_peers() -> Vec<super::patterns_external::CrossOsPeer> {
    super::patterns_external::discover_peers_stub()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_ambient() {
        let g = MembraneGrant {
            caps_bits: 1, // INFER only
            ..Default::default()
        };
        assert_eq!(graft(0xFFFF, &g), 1);
    }

    #[test]
    fn mcp_deny_by_default() {
        let g = MembraneGrant::default();
        assert!(!mcp_allowed(&g, "github"));
    }

    #[test]
    fn discover_peers_stub_nonempty() {
        let peers = discover_peers();
        assert!(peers.len() >= 3);
    }
}
