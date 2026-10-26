//! P.15 — BGE embed honesty: lexical fallback only; `BgeStatus::Absent` until real blob.

use nos_core::ModelSlot;

use crate::hub::ModelHub;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgeStatus {
    /// Nenhum blob BGE/reranker registado — não claim semantic cosine.
    Absent,
    /// Hub tem path Reranker/BGE mas infer embedding real = AWAITING wiring.
    BlobPresentInferAbsent,
    /// Fallback trigram hash — NÃO é embedding semântico.
    LexicalFallbackOnly,
}

pub fn bge_status(hub: &ModelHub) -> BgeStatus {
    if hub.path_of(ModelSlot::Reranker).is_some() {
        BgeStatus::BlobPresentInferAbsent
    } else {
        BgeStatus::Absent
    }
}

/// Fallback determinístico (FNV trigram buckets) — honesty: not semantic BGE.
pub fn embed_lexical_fallback(text: &str, dims: usize) -> Vec<f32> {
    let dims = dims.clamp(8, 384);
    let mut v = vec![0f32; dims];
    let t = text.to_ascii_lowercase();
    let bytes = t.as_bytes();
    if bytes.len() < 3 {
        return v;
    }
    for w in bytes.windows(3) {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in w {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100000001b3);
        }
        let idx = (h as usize) % dims;
        v[idx] += 1.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_without_blob() {
        let hub = ModelHub::new();
        assert_eq!(bge_status(&hub), BgeStatus::Absent);
    }

    #[test]
    fn lexical_fallback_normalized() {
        let e = embed_lexical_fallback("hello world test", 16);
        assert_eq!(e.len(), 16);
        let sum_sq: f32 = e.iter().map(|x| x * x).sum();
        assert!((sum_sq - 1.0).abs() < 0.01 || sum_sq == 0.0);
    }
}
