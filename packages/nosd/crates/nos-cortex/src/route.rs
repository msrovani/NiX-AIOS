//! Roteamento MoE Sistema 1 / Sistema 2 (manifesto NixAIOS Cortex/Hermes).
//! S1 = reação local (Active / experts leves). S2 = raciocínio profundo (GeneratorPro).
//! Onda 5: tenta `score_v0` (pesos fixos); senão keyword. Honesty: ≠ ROUTER.BITNET treinado.

use nos_core::{ModelSlot, S1Backend, s1_backend_effective, score_intent_s1};
use super::router_score::{decide_from_scores, score_intent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemLane {
    /// Latência-alvo local; nunca bloqueia heartbeat com trabalho pesado.
    S1Local,
    /// Worker dedicada (`S2Queue`); chat espera, tick S1 não partilha o lock.
    S2Deep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteDecision {
    pub slot: ModelSlot,
    pub lane: SystemLane,
    pub reason: &'static str,
}

/// Classifica intenção → slot + lane. Fail-closed: default S1 Active.
/// W3: `NOSD_S1_BACKEND=agentjev` sem `NOSD_S1_BLOB` → keyword efectivo (não rede neural fantasma).
pub fn route_intent(prompt: &str) -> RouteDecision {
    if s1_backend_effective() == S1Backend::Keyword {
        if let Some(d) = route_from_s1_keyword(prompt) {
            return d;
        }
    }
    if let Some(d) = decide_from_scores(score_intent(prompt)) {
        return d;
    }
    route_intent_keyword(prompt)
}

fn route_from_s1_keyword(prompt: &str) -> Option<RouteDecision> {
    let d = score_intent_s1(prompt);
    if d.is_abstain() {
        return None;
    }
    let slot = d.value?;
    let lane = if slot == ModelSlot::GeneratorPro {
        SystemLane::S2Deep
    } else {
        SystemLane::S1Local
    };
    Some(RouteDecision {
        slot,
        lane,
        reason: "s1_keyword",
    })
}

/// Keyword clássico (Onda 3) — usado quando score_v0 sem margem.
pub fn route_intent_keyword(prompt: &str) -> RouteDecision {
    let p = prompt.to_ascii_lowercase();
    let complex = p.len() > 280
        || contains_any(
            &p,
            &[
                "refator",
                "refactor",
                "arquitetur",
                "architecture",
                "implemente",
                "implement ",
                "desenvolva",
                "analise profunda",
                "análise profunda",
                "prove que",
                "theorem",
                "migrate",
                "migrar",
            ],
        );

    // Conector MoE Vision — LOO/slot Vision; infer real = AWAITING blob
    if contains_any(
        &p,
        &[
            "imagem",
            "image",
            "foto",
            "photo",
            "câmera",
            "camera",
            "webcam",
            "gesto",
            "gesture",
            "visão",
            "visao",
            "vision",
            "o que estou a ver",
            "o que estou vendo",
            "describe the frame",
        ],
    ) {
        return RouteDecision {
            slot: ModelSlot::Vision,
            lane: SystemLane::S1Local,
            reason: "vision_keywords",
        };
    }
    if contains_any(&p, &["pci", "hwid", "driver", "firmware", "gpu ", "nvidia", "amdgpu"]) {
        return RouteDecision {
            slot: ModelSlot::HwExpert,
            lane: SystemLane::S1Local,
            reason: "hw_keywords",
        };
    }
    if contains_any(&p, &["rust ", "cargo ", "crate ", "wasm", "borrow"]) {
        return RouteDecision {
            slot: ModelSlot::RustCoder,
            lane: if complex {
                SystemLane::S2Deep
            } else {
                SystemLane::S1Local
            },
            reason: "rust_keywords",
        };
    }
    if complex {
        return RouteDecision {
            slot: ModelSlot::GeneratorPro,
            lane: SystemLane::S2Deep,
            reason: "complex_or_long",
        };
    }
    RouteDecision {
        slot: ModelSlot::Active,
        lane: SystemLane::S1Local,
        reason: "default_s1",
    }
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vision_keywords() {
        let d = route_intent("o que a camera esta a ver");
        assert_eq!(d.slot, ModelSlot::Vision);
        assert_eq!(d.reason, "vision_keywords");
    }

    #[test]
    fn greet_is_s1_active() {
        let d = route_intent("oi");
        assert_eq!(d.slot, ModelSlot::Active);
        assert_eq!(d.lane, SystemLane::S1Local);
        assert!(d.reason == "score_v0_greet" || d.reason == "default_s1");
    }

    #[test]
    fn long_arch_is_s2_pro() {
        let d = route_intent(
            "refatore a arquitetura do cortex para migrar o hub MoE e prove que o roteamento \
             Sistema 2 não bloqueia o heartbeat do scheduler no NixAIOS appliance",
        );
        assert_eq!(d.slot, ModelSlot::GeneratorPro);
        assert_eq!(d.lane, SystemLane::S2Deep);
    }

    #[test]
    fn pci_to_hw() {
        let d = route_intent("qual driver nvidia para este pci");
        assert_eq!(d.slot, ModelSlot::HwExpert);
    }

    #[test]
    fn s1_keyword_env_routes_hw() {
        std::env::set_var("NOSD_S1_BACKEND", "keyword");
        let d = route_intent("nvidia pci driver firmware");
        std::env::remove_var("NOSD_S1_BACKEND");
        assert_eq!(d.reason, "s1_keyword");
        assert_eq!(d.slot, ModelSlot::HwExpert);
    }
}
