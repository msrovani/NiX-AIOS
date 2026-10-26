//! Decision<T> calibrado (ADR-0106 / S1) — abstain primeira classe.
//!
//! `NOSD_S1_BACKEND=keyword` → `score_intent_s1` (handcrafted, não AgentJev).
//! `jev`/`laya` → backend enum set, mas **runtime blob AWAITING** — rota usa score_v0/keyword, não rede neural.

use crate::ModelSlot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstainReason {
    ScorerAbsent,
    LowConfidence,
    NetAbsent,
    HitlRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision<T> {
    pub value: Option<T>,
    /// Q8 confidence 0..=255
    pub confidence_q8: u8,
    pub margin_q8: u8,
    pub abstain: Option<AbstainReason>,
    pub source: String,
}

impl<T> Decision<T> {
    pub fn decide(value: T, confidence_q8: u8, margin_q8: u8, source: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            confidence_q8,
            margin_q8,
            abstain: None,
            source: source.into(),
        }
    }

    pub fn abstain(reason: AbstainReason, source: impl Into<String>) -> Self {
        Self {
            value: None,
            confidence_q8: 0,
            margin_q8: 0,
            abstain: Some(reason),
            source: source.into(),
        }
    }

    pub fn is_abstain(&self) -> bool {
        self.abstain.is_some() || self.value.is_none()
    }
}

/// S1 scorer posture — AgentJev/Laya destilado ainda ausente ⇒ ScorerAbsent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum S1Backend {
    Keyword,
    DestilledLaya,
    AgentJev,
    Absent,
}

pub fn s1_backend_from_env() -> S1Backend {
    match std::env::var("NOSD_S1_BACKEND")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "laya" | "destilled" => S1Backend::DestilledLaya,
        "jev" | "agentjev" => S1Backend::AgentJev,
        "keyword" => S1Backend::Keyword,
        _ => S1Backend::Absent,
    }
}

/// W3 Master Unification: `agentjev`/`laya` só se blob runtime existir.
/// Sem ficheiro → Keyword (nunca fingir scorer neural).
/// Paths: `NOSD_S1_BLOB` ou LOO convencional sob `/var/lib/nosd/models/`.
pub fn s1_blob_present() -> bool {
    if let Ok(p) = std::env::var("NOSD_S1_BLOB") {
        let t = p.trim();
        if !t.is_empty() && std::path::Path::new(t).is_file() {
            return true;
        }
    }
    for cand in [
        "/var/lib/nosd/models/agentjev.gguf",
        "/var/lib/nosd/models/AgentJev.gguf",
        "/var/lib/nosd/models/AGENTJEV.GGUF",
    ] {
        if std::path::Path::new(cand).is_file() {
            return true;
        }
    }
    false
}

/// Backend efectivo para roteamento: AgentJev/Laya sem blob → Keyword.
pub fn s1_backend_effective() -> S1Backend {
    match s1_backend_from_env() {
        S1Backend::AgentJev | S1Backend::DestilledLaya if !s1_blob_present() => S1Backend::Keyword,
        other => other,
    }
}

pub fn s1_backend_label() -> &'static str {
    match s1_backend_from_env() {
        S1Backend::AgentJev | S1Backend::DestilledLaya if !s1_blob_present() => "scorer_absent→keyword",
        S1Backend::AgentJev => "agentjev",
        S1Backend::DestilledLaya => "laya",
        S1Backend::Keyword => "keyword",
        S1Backend::Absent => "absent",
    }
}

const S1_MIN_CONF: u8 = 140;
const S1_MIN_MARGIN: u8 = 24;

/// P.3 — keyword/trigram S1 scorer → `Decision<ModelSlot>` (honest substitute until AgentJev blob).
pub fn score_intent_s1(prompt: &str) -> Decision<ModelSlot> {
    let p = prompt.to_ascii_lowercase();
    let mut scores = [
        (ModelSlot::HwExpert, 0u16),
        (ModelSlot::RustCoder, 0u16),
        (ModelSlot::GeneratorPro, 0u16),
        (ModelSlot::Vision, 0u16),
        (ModelSlot::Reranker, 0u16),
        (ModelSlot::Active, 0u16),
    ];
    let mut bump = |slot: ModelSlot, n: u16| {
        let idx = match slot {
            ModelSlot::HwExpert => 0,
            ModelSlot::RustCoder => 1,
            ModelSlot::GeneratorPro => 2,
            ModelSlot::Vision => 3,
            ModelSlot::Reranker => 4,
            _ => 5,
        };
        scores[idx].1 = scores[idx].1.saturating_add(n);
    };

    for (w, n) in [
        ("pci", 4),
        ("driver", 3),
        ("firmware", 3),
        ("nvidia", 3),
        ("gpu", 2),
    ] {
        if p.contains(w) {
            bump(ModelSlot::HwExpert, n);
        }
    }
    for w in ["rust ", "cargo ", "crate ", "borrow", "wasm"] {
        if p.contains(w) {
            bump(ModelSlot::RustCoder, 3);
        }
    }
    for w in [
        "refator",
        "architecture",
        "implemente",
        "migrate",
        "analise profunda",
    ] {
        if p.contains(w) {
            bump(ModelSlot::GeneratorPro, 4);
        }
    }
    if p.len() > 200 {
        bump(ModelSlot::GeneratorPro, 2);
    }
    for w in ["camera", "foto", "imagem", "vision", "webcam"] {
        if p.contains(w) {
            bump(ModelSlot::Vision, 3);
        }
    }
    for w in ["rerank", "relevance", "bge"] {
        if p.contains(w) {
            bump(ModelSlot::Reranker, 3);
        }
    }
    for w in ["oi", "ola", "hello", "hey"] {
        if p == w || p.starts_with(&format!("{w} ")) {
            bump(ModelSlot::Active, 4);
        }
    }
    // trigram overlap with slot names (light)
    for (slot, tag) in [
        (ModelSlot::HwExpert, "hwexpert"),
        (ModelSlot::RustCoder, "rust"),
        (ModelSlot::Vision, "vis"),
    ] {
        if trigram_hits(&p, tag) > 0 {
            bump(slot, 2);
        }
    }

    let mut best = scores[5];
    let mut second = 0u16;
    for s in scores {
        if s.1 > best.1 {
            second = best.1;
            best = s;
        } else if s.1 > second {
            second = s.1;
        }
    }
    let conf = best.1.saturating_mul(12).min(255) as u8;
    let margin = best.1.saturating_sub(second).saturating_mul(12).min(255) as u8;
    if conf < S1_MIN_CONF || margin < S1_MIN_MARGIN {
        return Decision::abstain(AbstainReason::LowConfidence, "s1_keyword");
    }
    Decision::decide(best.0, conf, margin, "s1_keyword")
}

fn trigram_hits(hay: &str, needle: &str) -> u16 {
    let b = needle.as_bytes();
    if b.len() < 3 {
        return 0;
    }
    let mut n = 0u16;
    for w in hay.as_bytes().windows(3) {
        if w == &b[..3] {
            n = n.saturating_add(1);
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstain_scorer_absent() {
        let d: Decision<bool> = Decision::abstain(AbstainReason::ScorerAbsent, "s1");
        assert!(d.is_abstain());
    }

    #[test]
    fn decide_ok() {
        let d = Decision::decide(true, 200, 40, "keyword");
        assert!(!d.is_abstain());
        assert_eq!(d.value, Some(true));
    }

    #[test]
    fn s1_keyword_hw_slot() {
        let d = score_intent_s1("nvidia pci driver firmware");
        assert!(!d.is_abstain());
        assert_eq!(d.value, Some(ModelSlot::HwExpert));
    }

    #[test]
    fn s1_keyword_noise_abstains() {
        let d = score_intent_s1("x");
        assert!(d.is_abstain());
    }

    #[test]
    fn agentjev_without_blob_falls_to_keyword() {
        std::env::set_var("NOSD_S1_BACKEND", "agentjev");
        std::env::remove_var("NOSD_S1_BLOB");
        assert_eq!(s1_backend_effective(), S1Backend::Keyword);
        assert!(s1_backend_label().contains("keyword"));
        std::env::remove_var("NOSD_S1_BACKEND");
    }
}
