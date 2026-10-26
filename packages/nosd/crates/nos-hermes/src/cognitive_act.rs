//! CognitiveAct — Observe→Plan→Act→Verify→Remember (OPAR) com persistência honesta.
//! NX.6: campo `seek_audit` (sought→hit|miss|synth|NetAbsent) quando o ciclo envolve Factory.

use nos_memory::{MemoryRecord, NosMemory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CognitiveAct {
    pub domain: String,
    pub observe_note: String,
    pub plan_note: String,
    pub act_note: String,
    pub verify_note: String,
    pub remembered: bool,
    /// F.3 / NX.6 — linha de auditoria Seek quando aplicável.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seek_audit: Option<String>,
}

/// Encerra ciclo OPAR; `remembered=true` só se `memory.remember` ok.
pub fn wrap_opar(
    memory: &NosMemory,
    domain: &str,
    observe_note: &str,
    plan_note: &str,
    act_note: &str,
    verify_note: &str,
) -> CognitiveAct {
    wrap_opar_with_seek(
        memory,
        domain,
        observe_note,
        plan_note,
        act_note,
        verify_note,
        None,
    )
}

/// OPAR com seek audit (NX.6) — Factory/AutoLearn/spawn.
pub fn wrap_opar_with_seek(
    memory: &NosMemory,
    domain: &str,
    observe_note: &str,
    plan_note: &str,
    act_note: &str,
    verify_note: &str,
    seek_audit: Option<&str>,
) -> CognitiveAct {
    let seek_part = seek_audit
        .map(|s| format!(" | seek:{s}"))
        .unwrap_or_default();
    let text = format!(
        "observe:{observe_note} | plan:{plan_note} | act:{act_note} | verify:{verify_note}{seek_part}"
    );
    let key = format!("md/L3/opar/{}", Uuid::new_v4());
    let remembered = memory
        .remember(MemoryRecord {
            key: key.clone(),
            text,
            scope: format!("session/{domain}"),
            entities: vec!["mom/pattern".into(), format!("domain/{domain}")],
        })
        .is_ok();
    CognitiveAct {
        domain: domain.to_string(),
        observe_note: observe_note.to_string(),
        plan_note: plan_note.to_string(),
        act_note: act_note.to_string(),
        verify_note: verify_note.to_string(),
        remembered,
        seek_audit: seek_audit.map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn wrap_opar_remembered() {
        let d = tempdir().unwrap();
        let mem = NosMemory::open(d.path()).unwrap();
        let act = wrap_opar(&mem, "mesh", "peers=2", "route", "send", "ok");
        assert!(act.remembered);
        assert_eq!(act.domain, "mesh");
        assert!(act.seek_audit.is_none());
        let hits = mem.recall("route", 3).unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    fn wrap_opar_includes_seek_audit() {
        let d = tempdir().unwrap();
        let mem = NosMemory::open(d.path()).unwrap();
        let act = wrap_opar_with_seek(
            &mem,
            "factory.seek",
            "goal=x",
            "sources",
            "sought→synth",
            "ok",
            Some("sought→Synth sources=[Local] hit=- synth=true"),
        );
        assert!(act.remembered);
        assert!(act.seek_audit.as_ref().unwrap().contains("sought"));
    }
}
