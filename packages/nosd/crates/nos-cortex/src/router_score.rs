//! Router score v0 — features lexicais + pesos fixos (não é MoE treinado).
//! Honesty: `score_v0` ≠ ROUTER.BITNET / Trinity neural; fallback keyword se margem baixa.

use super::route::{RouteDecision, SystemLane};
use nos_core::ModelSlot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteScores {
    pub hw: i32,
    pub rust: i32,
    pub complex: i32,
    pub greet: i32,
}

/// Conta hits de features; pesos fixos (handcrafted), sem treino.
pub fn score_intent(prompt: &str) -> RouteScores {
    let p = prompt.to_ascii_lowercase();
    let mut hw = 0i32;
    let mut rust = 0i32;
    let mut complex = 0i32;
    let mut greet = 0i32;

    for (w, n) in [
        ("pci", 3),
        ("hwid", 3),
        ("driver", 2),
        ("firmware", 2),
        ("nvidia", 2),
        ("amdgpu", 2),
        ("gpu ", 1),
    ] {
        if p.contains(w) {
            hw = hw.saturating_add(n);
        }
    }
    for (w, n) in [
        ("rust ", 3),
        ("cargo ", 2),
        ("crate ", 2),
        ("borrow", 2),
        ("wasm", 1),
    ] {
        if p.contains(w) {
            rust = rust.saturating_add(n);
        }
    }
    for (w, n) in [
        ("refator", 3),
        ("refactor", 3),
        ("arquitetur", 3),
        ("architecture", 3),
        ("implemente", 2),
        ("implement ", 2),
        ("desenvolva", 2),
        ("analise profunda", 3),
        ("análise profunda", 3),
        ("prove que", 2),
        ("theorem", 2),
        ("migrate", 2),
        ("migrar", 2),
    ] {
        if p.contains(w) {
            complex = complex.saturating_add(n);
        }
    }
    if p.len() > 280 {
        complex = complex.saturating_add(4);
    } else if p.len() > 120 {
        complex = complex.saturating_add(1);
    }
    for w in ["oi", "olá", "ola", "hello", "hey", "bom dia", "boa tarde"] {
        if p == w || p.starts_with(&format!("{w} ")) || p.starts_with(&format!("{w},")) {
            greet = greet.saturating_add(3);
        }
    }

    RouteScores {
        hw,
        rust,
        complex,
        greet,
    }
}

const MARGIN: i32 = 2;

/// Escolhe slot/lane por margem de score; `None` → caller usa keyword clássico.
pub fn decide_from_scores(s: RouteScores) -> Option<RouteDecision> {
    let candidates = [
        (s.hw, ModelSlot::HwExpert, SystemLane::S1Local, "score_v0_hw"),
        (
            s.rust + if s.complex >= 3 { 1 } else { 0 },
            ModelSlot::RustCoder,
            if s.complex >= 3 {
                SystemLane::S2Deep
            } else {
                SystemLane::S1Local
            },
            "score_v0_rust",
        ),
        (
            s.complex,
            ModelSlot::GeneratorPro,
            SystemLane::S2Deep,
            "score_v0_complex",
        ),
        (s.greet, ModelSlot::Active, SystemLane::S1Local, "score_v0_greet"),
    ];
    let mut best = candidates[0];
    for c in candidates.iter().skip(1) {
        if c.0 > best.0 {
            best = *c;
        }
    }
    let second = candidates
        .iter()
        .filter(|c| c.1 != best.1 || c.2 != best.2)
        .map(|c| c.0)
        .max()
        .unwrap_or(0);
    if best.0 < MARGIN || best.0 - second < MARGIN {
        return None;
    }
    Some(RouteDecision {
        slot: best.1,
        lane: best.2,
        reason: best.3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_scores() {
        let s = score_intent("oi");
        assert!(s.greet >= 3);
        let d = decide_from_scores(s).expect("greet margin");
        assert_eq!(d.slot, ModelSlot::Active);
        assert_eq!(d.reason, "score_v0_greet");
    }

    #[test]
    fn hw_beats_greet() {
        let s = score_intent("nvidia pci driver firmware");
        assert!(s.hw >= 5);
        let d = decide_from_scores(s).unwrap();
        assert_eq!(d.slot, ModelSlot::HwExpert);
    }

    #[test]
    fn ambiguous_returns_none() {
        // short noise — no clear winner
        assert!(decide_from_scores(score_intent("x")).is_none());
    }
}
