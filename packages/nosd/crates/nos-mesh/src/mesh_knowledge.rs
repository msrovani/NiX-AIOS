//! Dedup / fingerprint para mesh knowledge (ADR-0081 parity).
//!
//! 0.5.0: merge CRDT thin em [`crate::crdt`] (LWW + VV). Sync P2P NSGDB pleno /
//! lab 2-nó e2e = residual AWAITING_HW (REMIGRATION M.2).

/// FNV-1a 64-bit sobre bytes de conteúdo (mesmo algoritmo que `nos-factory` body hash).
pub fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut h = OFFSET;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(PRIME);
    }
    h
}

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
