//! AutoLearn helicóptero (T.2) — miss → Seek → import|synth|fetch → remember.

use nos_factory::{seek_skill, SeekConfig, SeekReport, SkillNeed};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LearnAction {
    SeekHit,
    Synthesize,
    FetchModel,
    AbstainNet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnPlan {
    pub need: String,
    pub action: LearnAction,
    pub seek: SeekReport,
    pub note: String,
}

/// Deriva ação a partir de um SeekReport já executado (Hermes passa mesh+índice).
pub fn plan_from_seek(goal: &str, seek: SeekReport) -> LearnPlan {
    if seek.hit.is_some() {
        return LearnPlan {
            need: goal.into(),
            action: LearnAction::SeekHit,
            seek,
            note: "reuse imported wasmi/skill".into(),
        };
    }
    if !seek.synthesize_allowed {
        return LearnPlan {
            need: goal.into(),
            action: LearnAction::AbstainNet,
            seek,
            note: "Seek incomplete — do not fake generate".into(),
        };
    }
    LearnPlan {
        need: goal.into(),
        action: LearnAction::Synthesize,
        seek,
        note: "Seek exhausted — Factory path A last resort".into(),
    }
}

/// Plano AutoLearn: sempre Seek primeiro (F.3) — públicos incluídos (frugal só reduz volume).
pub fn plan_learn(goal: &str, net_disabled: bool) -> LearnPlan {
    let need = SkillNeed {
        goal: goal.into(),
        capabilities: vec![goal.into()],
        frugal_budget: true,
    };
    let cfg = SeekConfig {
        net_disabled,
        private_allowlist_urls: std::env::var("NOSD_SEEK_PRIVATE_URLS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|u| u.trim().to_string())
                    .filter(|u| !u.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        ..Default::default()
    };
    plan_from_seek(goal, seek_skill(&need, &cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_absent_abstains() {
        let p = plan_learn("pilotar helicoptero", true);
        assert_eq!(p.action, LearnAction::AbstainNet);
    }

    #[test]
    fn net_ok_can_synth() {
        let p = plan_learn("obscure niche skill xyzzy", false);
        assert!(matches!(
            p.action,
            LearnAction::Synthesize | LearnAction::SeekHit
        ));
    }
}
