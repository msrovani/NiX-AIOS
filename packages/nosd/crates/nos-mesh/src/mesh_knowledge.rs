//! Dedup / fingerprint para mesh knowledge (ADR-0081 parity).
//!
//! 0.5.0: merge CRDT thin em [`crate::crdt`] (LWW + VV). Sync P2P NSGDB pleno /
//! lab 2-nó e2e = residual AWAITING_HW (REMIGRATION M.2).

/// Re-export — implementação canónica em `nos-core::hash`.
pub use nos_core::fnv1a64;

/// Fingerprint estável para dedupe TX/RX (conteúdo UTF-8, sem nonce).
pub fn content_fingerprint(text: &str) -> u64 {
    fnv1a64(text.trim().as_bytes())
}

/// Mesmo fingerprint para payload binário (ex. FRAG reassembled).
pub fn payload_fingerprint(payload: &[u8]) -> u64 {
    fnv1a64(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_stable() {
        let a = content_fingerprint("hello mesh");
        let b = content_fingerprint("hello mesh");
        assert_eq!(a, b);
        assert_ne!(a, content_fingerprint("other"));
    }

    #[test]
    fn trim_affects_fingerprint() {
        assert_eq!(
            content_fingerprint("  x  "),
            content_fingerprint("x")
        );
    }
}
