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
}
