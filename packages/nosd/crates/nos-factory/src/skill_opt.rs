//! F.2 — SelfEvolve / SkillOpt thin (não só PathBcAbsent): Seek → draft → CapGate AGENT_SPAWN.

use crate::{ArtifactDraft, FactoryPath, SeekReport, SkillNeed};
use serde::{Deserialize, Serialize};

const SCORE_PROMOTE: f32 = 0.72;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillOptAction {
    ReuseSeekHit,
    ProposeSkillOpt,
    SelfEvolveAgentMd,
    AbstainLowScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOptPlan {
    pub need: String,
    pub action: SkillOptAction,
    pub candidate_score: f32,
    pub draft: Option<ArtifactDraft>,
    pub agent_md: Option<String>,
    pub note: String,
}

/// Observe skill miss → seek report → propose mutate draft se score alto.
pub fn plan_skill_opt(need: &SkillNeed, seek: &SeekReport) -> SkillOptPlan {
    let score = seek
        .hit
        .as_ref()
        .map(|h| h.score)
        .or_else(|| seek.candidates.first().map(|c| c.score))
        .unwrap_or(0.0);

    if let Some(hit) = seek.hit.as_ref() {
        if hit.score >= SCORE_PROMOTE {
            return SkillOptPlan {
                need: need.goal.clone(),
                action: SkillOptAction::ReuseSeekHit,
                candidate_score: hit.score,
                draft: None,
                agent_md: None,
                note: "Seek hit — import/store.install, skip synth".into(),
            };
        }
    }

    if score >= SCORE_PROMOTE && seek.synthesize_allowed {
        let name = sanitize_name(&need.goal);
        let draft = ArtifactDraft {
            name: name.clone(),
            wasm: vec![],
            path: FactoryPath::AWasmi,
        };
        let md = self_evolve_agent_md(&name, 1);
        return SkillOptPlan {
            need: need.goal.clone(),
            action: SkillOptAction::ProposeSkillOpt,
            candidate_score: score,
            draft: Some(draft),
            agent_md: Some(md),
            note: "SkillOpt draft — requires CapGate AGENT_SPAWN + validate path A".into(),
        };
    }

    if need.goal.contains("agent") || need.goal.contains("self") {
        let name = sanitize_name(&need.goal);
        return SkillOptPlan {
            need: need.goal.clone(),
            action: SkillOptAction::SelfEvolveAgentMd,
            candidate_score: score,
            draft: None,
            agent_md: Some(self_evolve_agent_md(&name, 2)),
            note: "SelfEvolve — bump version on AGENT.md draft only (no full evolve loop)".into(),
        };
    }

    SkillOptPlan {
        need: need.goal.clone(),
        action: SkillOptAction::AbstainLowScore,
        candidate_score: score,
        draft: None,
        agent_md: None,
        note: "score below promote threshold — honest abstain".into(),
    }
}

/// Bump `version:` no frontmatter de rascunho AGENT.md (SelfEvolve thin).
pub fn self_evolve_agent_md(name: &str, version: u32) -> String {
    format!(
        r#"---
schema: 1
kind: agent
name: {name}
version: {version}
goal: evolve capability
contexto: SkillOpt/SelfEvolve thin NixAIOS
acionaveis: [event_driven]
---
## Contexto
Draft SelfEvolve — não assinado, não fleet-active.
## Goal
Increment version field only (F.2 honesty).
## Acionaveis
event_driven
## Workflow
Observe→Plan→Act→Verify
## Pre-Flight
CapGate AGENT_SPAWN
## Success Criteria
validate_artifact_md pass
## Failure Policy
escalate HITL
"#
    )
}

fn sanitize_name(goal: &str) -> String {
    let s: String = goal
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let t = s.trim_matches('_');
    if t.is_empty() {
        "skill_opt".into()
    } else {
        t.chars().take(48).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seek::{CandidateSource, SkillCandidate};

    #[test]
    fn plan_skill_opt_high_score_proposes_draft() {
        let need = SkillNeed {
            goal: "wifi mesh bridge".into(),
            capabilities: vec![],
            frugal_budget: true,
        };
        let seek = SeekReport {
            sources_tried: vec![],
            candidates: vec![SkillCandidate {
                source: CandidateSource::Local,
                name: "wifi".into(),
                url: "local://wifi".into(),
                score: 0.9,
                license: None,
                escalate_license: false,
            }],
            hit: None,
            synthesize_allowed: true,
            synthesize_reason: None,
            outcome: crate::SeekOutcome::Miss,
        };
        let p = plan_skill_opt(&need, &seek);
        assert_eq!(p.action, SkillOptAction::ProposeSkillOpt);
        assert!(p.draft.is_some());
    }

    #[test]
    fn plan_skill_opt_low_score_abstains() {
        let need = SkillNeed {
            goal: "x".into(),
            capabilities: vec![],
            frugal_budget: true,
        };
        let seek = SeekReport {
            sources_tried: vec![],
            candidates: vec![],
            hit: None,
            synthesize_allowed: false,
            synthesize_reason: Some("net".into()),
            outcome: crate::SeekOutcome::NetAbsent,
        };
        let p = plan_skill_opt(&need, &seek);
        assert_eq!(p.action, SkillOptAction::AbstainLowScore);
    }

    #[test]
    fn self_evolve_bumps_version() {
        let md = self_evolve_agent_md("demo", 3);
        assert!(md.contains("version: 3"));
    }
}
