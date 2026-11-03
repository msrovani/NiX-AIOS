//! CHJ ImprovePropose — card HITL server-authored (kind + fingerprint TTL).
//! Ring Nix desarmado: `accept_effect` descreve efeitos honestos, sem auto-nix.

use nos_core::{CapBits, CapGate, UiSource, WindowSpec};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::window_broker::WindowBroker;

/// Proposta de melhoria (brain, skill, kernel, …) serializável em RPC/CRDT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImprovePropose {
    pub kind: String,
    pub title: String,
    pub reason: String,
    pub importance: String,
    pub current: String,
    pub candidate: String,
    pub evidence: Vec<String>,
    pub alternatives: Vec<String>,
    pub grant_kind: String,
    pub accept_effect: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposeOutcome {
    pub window_id: u64,
    pub opened: bool,
    /// Card não reaberta — mesmo fingerprint dentro do TTL.
    pub skipped_ttl: bool,
    pub fingerprint: String,
}

pub const DEFAULT_FP_TTL_MS: u64 = 24 * 60 * 60 * 1000;

/// Hash estável: kind + rótulo do candidato + versão (campo `version` ou sufixo).
pub fn fingerprint_of(p: &ImprovePropose) -> String {
    let label = candidate_label(&p.candidate);
    let version = candidate_version(&p.candidate);
    let material = format!("{}|{}|{}", p.kind.trim(), label, version);
    blake3::hash(material.as_bytes()).to_hex().to_string()
}

fn candidate_label(candidate: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
        return v
            .get("label")
            .or_else(|| v.get("name"))
            .or_else(|| v.get("template_id"))
            .and_then(|x| x.as_str())
            .unwrap_or(candidate)
            .trim()
            .to_string();
    }
    candidate.trim().to_string()
}

fn candidate_version(candidate: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
        return v
            .get("version")
            .and_then(|x| x.as_str())
            .unwrap_or("1")
            .trim()
            .to_string();
    }
    "1".into()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn fp_recent(fp_cache: &Mutex<HashMap<String, u64>>, fp: &str, ttl_ms: u64) -> bool {
    let now = now_ms();
    let map = fp_cache.lock();
    map.get(fp).map(|ts| *ts + ttl_ms > now).unwrap_or(false)
}

fn fp_remember(fp_cache: &Mutex<HashMap<String, u64>>, fp: &str) {
    fp_cache.lock().insert(fp.to_string(), now_ms());
}

/// Corpo da card: razão + efeito ao aceitar + bytes/candidato (sem inventar updates).
pub fn card_body(p: &ImprovePropose) -> String {
    let mut lines = vec![
        p.reason.clone(),
        String::new(),
        format!("Importância: {}", p.importance),
        format!("Actual: {}", p.current),
        format!("Candidato: {}", p.candidate),
    ];
    if !p.evidence.is_empty() {
        lines.push(String::new());
        lines.push("Evidência:".into());
        for e in &p.evidence {
            lines.push(format!("  - {e}"));
        }
    }
    if !p.alternatives.is_empty() {
        lines.push(String::new());
        lines.push("Alternativas:".into());
        for a in &p.alternatives {
            lines.push(format!("  - {a}"));
        }
    }
    lines.push(String::new());
    lines.push(format!("Se aceitar (grant `{}`):", p.grant_kind));
    lines.push(p.accept_effect.clone());
    lines.join("\n")
}

/// Abre `WindowSpec::hitl_confirm` salvo fingerprint recente no cache in-memory.
pub fn propose(
    windows: &WindowBroker,
    _gate: &CapGate,
    cap_gen: u64,
    mut p: ImprovePropose,
    fp_cache: &Mutex<HashMap<String, u64>>,
    ttl_ms: u64,
) -> Result<ProposeOutcome, &'static str> {
    if p.fingerprint.is_empty() {
        p.fingerprint = fingerprint_of(&p);
    }
    let fp = p.fingerprint.clone();
    if fp_recent(fp_cache, &fp, ttl_ms) {
        return Ok(ProposeOutcome {
            window_id: 0,
            opened: false,
            skipped_ttl: true,
            fingerprint: fp,
        });
    }
    let wid = windows.alloc_id();
    let body = card_body(&p);
    let mut spec = WindowSpec::hitl_confirm(wid, p.title.clone(), body, UiSource::Hermes);
    spec.grant_kind = p.grant_kind.clone();
    let gate_hitl = CapGate::new(CapBits::HITL, cap_gen.max(1));
    windows.open(&gate_hitl, cap_gen, spec)?;
    fp_remember(fp_cache, &fp);
    Ok(ProposeOutcome {
        window_id: wid,
        opened: true,
        skipped_ttl: false,
        fingerprint: fp,
    })
}

/// Helper OPAR quando o operador nega a card (CorrectPlan — só metadados).
pub fn correct_plan_on_deny(p: &ImprovePropose, deny_reason: &str) -> serde_json::Value {
    let fp = if p.fingerprint.is_empty() {
        fingerprint_of(p)
    } else {
        p.fingerprint.clone()
    };
    json!({
        "opar_phase": "plan_correct",
        "domain": "improve",
        "kind": p.kind,
        "fingerprint": fp,
        "grant_kind": p.grant_kind,
        "deny_reason": deny_reason,
        "candidate": p.candidate,
        "note": "operador negou HITL — sem act side-effect; ring Nix continua desarmado",
    })
}

/// Converte proposta brain upgrade para ImprovePropose (card unificada).
pub fn from_brain_proposal(
    plan: &crate::brain_upgrade::BrainUpgradePlan,
    pr: &crate::brain_upgrade::Proposal,
) -> ImprovePropose {
    let hitl_kind = match pr.direction {
        crate::brain_upgrade::AdaptDirection::Downgrade => "brain.downgrade",
        _ => "brain.upgrade",
    };
    let candidate = serde_json::json!({
        "label": pr.candidate.name,
        "version": format!("tier{}", pr.tier),
        "repo": pr.candidate.repo,
        "file": pr.candidate.file,
        "size_bytes": pr.candidate.size_bytes,
        "sha256": pr.candidate.sha256,
    })
    .to_string();
    let ring_note = if plan.observed.ring_armed {
        "ring armado (lab)"
    } else {
        "ring Nix desarmado (default)"
    };
    let mut p = ImprovePropose {
        kind: "brain".into(),
        title: format!("Brain upgrade — tier {} ({})", pr.tier, pr.candidate.name),
        reason: format!(
            "{}\nGanho: {} — {}\nCusto: {} MB pinados (exacto)",
            plan.honesty, pr.gain.size_label, pr.gain.honesty_note, pr.cost.download_mb
        ),
        importance: "high".into(),
        current: format!(
            "{} ({} bytes)",
            plan.current.label,
            plan.current.bytes.unwrap_or(0)
        ),
        candidate,
        evidence: pr.requires.clone(),
        alternatives: vec![plan.rollback.clone()],
        grant_kind: hitl_kind.into(),
        accept_effect: format!(
            "Fetch GGUF pinado + hot_load após grant `{hitl_kind}`; {ring_note} — \
             sem nixos-rebuild automático no chat"
        ),
        fingerprint: String::new(),
    };
    p.fingerprint = fingerprint_of(&p);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use nos_bus::EventBus;

    #[test]
    fn fingerprint_stable_for_same_material() {
        let p = ImprovePropose {
            kind: "skill".into(),
            title: "t".into(),
            reason: "r".into(),
            importance: "med".into(),
            current: "none".into(),
            candidate: r#"{"label":"net.monitor","version":"1"}"#.into(),
            evidence: vec![],
            alternatives: vec![],
            grant_kind: "skill.install".into(),
            accept_effect: "install wasm".into(),
            fingerprint: String::new(),
        };
        let a = fingerprint_of(&p);
        let b = fingerprint_of(&p);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn propose_skips_second_open_within_ttl() {
        let bus = EventBus::new(8);
        let windows = WindowBroker::new(bus);
        let cache = Mutex::new(HashMap::new());
        let gate = CapGate::new(CapBits::HITL, 1);
        let p = ImprovePropose {
            kind: "skill".into(),
            title: "Skill".into(),
            reason: "test".into(),
            importance: "low".into(),
            current: "-".into(),
            candidate: r#"{"label":"echo.infer","version":"1"}"#.into(),
            evidence: vec![],
            alternatives: vec![],
            grant_kind: "skill.install".into(),
            accept_effect: "placeholder template".into(),
            fingerprint: String::new(),
        };
        let o1 = propose(&windows, &gate, 1, p.clone(), &cache, DEFAULT_FP_TTL_MS).unwrap();
        assert!(o1.opened);
        let o2 = propose(&windows, &gate, 1, p, &cache, DEFAULT_FP_TTL_MS).unwrap();
        assert!(!o2.opened);
        assert!(o2.skipped_ttl);
    }

    #[test]
    fn correct_plan_on_deny_json() {
        let p = ImprovePropose {
            kind: "kernel".into(),
            title: "k".into(),
            reason: "r".into(),
            importance: "med".into(),
            current: "6.6".into(),
            candidate: "6.12".into(),
            evidence: vec![],
            alternatives: vec![],
            grant_kind: "nix.apply".into(),
            accept_effect: "none auto".into(),
            fingerprint: String::new(),
        };
        let v = correct_plan_on_deny(&p, "user dismissed");
        assert_eq!(v["opar_phase"], "plan_correct");
        assert_eq!(v["deny_reason"], "user dismissed");
    }
}
