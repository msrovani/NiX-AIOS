//! P.10 — Vision / Reranker slot resolve (keywords); EngineAbsent se blob ausente no hub.

use nos_core::ModelSlot;

use crate::hub::ModelHub;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionRerankRole {
    Vision,
    Reranker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisionRerankResolve {
    Ready {
        role: VisionRerankRole,
        slot: ModelSlot,
        path: String,
    },
    KeywordsOnly {
        role: VisionRerankRole,
        slot: ModelSlot,
    },
    EngineAbsent,
}

pub fn role_from_prompt(prompt: &str) -> Option<VisionRerankRole> {
    let p = prompt.to_ascii_lowercase();
    if contains_any(
        &p,
        &[
            "rerank",
            "re-rank",
            "relevance",
            "bge",
            "embedding rank",
            "ordenar resultados",
        ],
    ) {
        return Some(VisionRerankRole::Reranker);
    }
    if contains_any(
        &p,
        &[
            "imagem",
            "image",
            "foto",
            "camera",
            "câmera",
            "webcam",
            "vision",
            "visão",
            "visao",
            "gesto",
        ],
    ) {
        return Some(VisionRerankRole::Vision);
    }
    None
}

pub fn resolve_vision_rerank(prompt: &str, hub: &ModelHub) -> VisionRerankResolve {
    let role = match role_from_prompt(prompt) {
        Some(r) => r,
        None => return VisionRerankResolve::EngineAbsent,
    };
    let slot = match role {
        VisionRerankRole::Vision => ModelSlot::Vision,
        VisionRerankRole::Reranker => ModelSlot::Reranker,
    };
    if let Some(path) = hub.path_of(slot) {
        return VisionRerankResolve::Ready {
            role,
            slot,
            path: path.display().to_string(),
        };
    }
    VisionRerankResolve::KeywordsOnly { role, slot }
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vision_keywords_without_hub() {
        let hub = ModelHub::new();
        let r = resolve_vision_rerank("descreva a foto da camera", &hub);
        assert!(matches!(
            r,
            VisionRerankResolve::KeywordsOnly {
                role: VisionRerankRole::Vision,
                ..
            }
        ));
    }

    #[test]
    fn no_keywords_absent() {
        let hub = ModelHub::new();
        assert!(matches!(
            resolve_vision_rerank("oi", &hub),
            VisionRerankResolve::EngineAbsent
        ));
    }
}
