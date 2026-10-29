//! SITE_POLICY — Auto | Confirm | Escalate | Deny (Tema NX.0 / ADR-0106).
//! Full-auto-auto ≠ root sem HITL: actos destrutivos nunca Auto por confiança.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActDisposition {
    /// Corre sem card HITL.
    Auto,
    /// Precisa Confirm na UI JARBAS.
    Confirm,
    /// Abster / Escalate humano (não actua).
    Escalate,
    /// Negado absoluto (imune a confiança).
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActDomain {
    ChatRoute,
    ModelPlanSuggest,
    ModelFetch,
    HotSwapKnownHash,
    HotSwapFirstPro,
    WifiScan,
    WifiConnect,
    PipeWireVolume,
    NixPropose,
    NixSwitch,
    NixRollback,
    DeviceFirmware,
    AgentSpawnSigned,
    AgentSpawnUnsigned,
    FactorySeekScan,
    FactorySeekInstall,
    FactorySynthesize,
    SeekHostOffAllowlist,
    SystemctlScoped,
    GpuPowerLimit,
    Repartition,
    MeshSkillSyncExec,
    /// Brain self-upgrade (ADR-0118 addenda 0118-qwen-embedded-brain):
    /// aceita proposta de modelo maior/melhor — HITL sempre.
    BrainUpgrade,
}

/// Política canónica NIXAIOS (prod). ISO lab pode relaxar NixSwitch via env.
pub fn disposition(domain: ActDomain, lab_auto_switch: bool) -> ActDisposition {
    use ActDisposition::*;
    use ActDomain::*;
    match domain {
        ChatRoute | ModelPlanSuggest | WifiScan | FactorySeekScan => Auto,
        HotSwapKnownHash | PipeWireVolume | SystemctlScoped => Auto,
        AgentSpawnSigned => Auto,
        ModelFetch | HotSwapFirstPro | FactorySeekInstall => Confirm,
        WifiConnect | NixPropose => Confirm,
        NixSwitch => {
            if lab_auto_switch {
                Auto
            } else {
                Confirm
            }
        }
        NixRollback | DeviceFirmware | FactorySynthesize => Escalate,
        SeekHostOffAllowlist | AgentSpawnUnsigned => Escalate,
        BrainUpgrade => Escalate,
        GpuPowerLimit | Repartition | MeshSkillSyncExec => Deny,
    }
}

/// ScorerAbsent / LowConfidence em acto → Escalate (nunca Auto).
pub fn with_abstain(base: ActDisposition, scorer_absent: bool) -> ActDisposition {
    if !scorer_absent {
        return base;
    }
    match base {
        ActDisposition::Auto | ActDisposition::Confirm => ActDisposition::Escalate,
        other => other,
    }
}

pub fn lab_auto_switch_from_env() -> bool {
    match std::env::var("NOSD_NIX_AUTO_SWITCH") {
        Ok(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

/// Matriz canónica (domínio → disposition em prod, lab_auto_switch=false).
pub fn policy_matrix_prod() -> Vec<(ActDomain, ActDisposition)> {
    use ActDomain::*;
    const DOMAINS: &[ActDomain] = &[
        ChatRoute,
        ModelPlanSuggest,
        ModelFetch,
        HotSwapKnownHash,
        HotSwapFirstPro,
        WifiScan,
        WifiConnect,
        PipeWireVolume,
        NixPropose,
        NixSwitch,
        NixRollback,
        DeviceFirmware,
        AgentSpawnSigned,
        AgentSpawnUnsigned,
        FactorySeekScan,
        FactorySeekInstall,
        FactorySynthesize,
        SeekHostOffAllowlist,
        SystemctlScoped,
        GpuPowerLimit,
        Repartition,
        MeshSkillSyncExec,
        BrainUpgrade,
    ];
    DOMAINS
        .iter()
        .copied()
        .map(|d| (d, disposition(d, false)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prod_switch_needs_confirm() {
        assert_eq!(
            disposition(ActDomain::NixSwitch, false),
            ActDisposition::Confirm
        );
    }

    #[test]
    fn lab_switch_auto() {
        assert_eq!(
            disposition(ActDomain::NixSwitch, true),
            ActDisposition::Auto
        );
    }

    #[test]
    fn synthesize_escalates() {
        assert_eq!(
            disposition(ActDomain::FactorySynthesize, false),
            ActDisposition::Escalate
        );
    }

    #[test]
    fn brick_paths_deny() {
        assert_eq!(
            disposition(ActDomain::Repartition, true),
            ActDisposition::Deny
        );
        assert_eq!(
            disposition(ActDomain::GpuPowerLimit, true),
            ActDisposition::Deny
        );
    }

    #[test]
    fn abstain_promotes_auto_to_escalate() {
        assert_eq!(
            with_abstain(ActDisposition::Auto, true),
            ActDisposition::Escalate
        );
    }

    #[test]
    fn matrix_prod_switch_confirm_repartition_deny() {
        let m = policy_matrix_prod();
        assert!(m.len() >= 20);
        assert_eq!(
            m.iter()
                .find(|(d, _)| *d == ActDomain::NixSwitch)
                .map(|(_, x)| *x),
            Some(ActDisposition::Confirm)
        );
        assert_eq!(
            m.iter()
                .find(|(d, _)| *d == ActDomain::Repartition)
                .map(|(_, x)| *x),
            Some(ActDisposition::Deny)
        );
    }
}
