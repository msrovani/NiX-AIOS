//! FNV-1a 64-bit — hash estável partilhado (factory + mesh). ADR anti dual-truth.

/// FNV-1a 64-bit sobre bytes.
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

pub fn fnv1a64_hex16(data: &[u8]) -> String {
    format!("{:016x}", fnv1a64(data))
}

/// Comparação em tempo constante para tokens (mesmo comprimento).
/// Comprimentos diferentes devolvem `false` sem vazar o primeiro byte divergente.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_stable() {
        let a = fnv1a64(b"hello mesh");
        assert_eq!(a, fnv1a64(b"hello mesh"));
        assert_ne!(a, fnv1a64(b"other"));
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64_hex16(b"x").len(), 16);
    }

    #[test]
    fn ct_eq_matches_only_identical_bytes() {
        assert!(ct_eq(b"token-123", b"token-123"));
        assert!(!ct_eq(b"token-123", b"token-124"));
        assert!(!ct_eq(b"token", b"token-123"));
        assert!(!ct_eq(b"", b"x"));
    }
}
