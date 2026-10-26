//! Padrões externos documentados (P.16) — referência Cross-OS, **sem fork** de código.
//!
//! | Fonte | Padrão | Uso em NixAIOS |
//! |-------|--------|----------------|
//! | [assistd](https://github.com/nornagon/assistd) (referência) | Daemon macOS + tool bridge | `nosd` IPC + CapGate server-authored |
//! | Flowork / workflow runners | Skill DAG + HITL gates | Hermes OPAR + `store.install` sandbox |
//! | [Anthropic Skills](https://github.com/anthropics/skills) | `SKILL.md` frontmatter + tool allowlist | ADR-0052 `validate_artifact_md` + Membrane MCP |
//!
//! Não copiamos repositórios terceiros; alinhamos contratos (manifest, deny-by-default, escala humana).

/// Identificadores estáveis para telemetria / discover_peers (Cross-OS).
pub const PATTERN_ASSISTD_DAEMON: &str = "cross_os/assistd-daemon-bridge";
pub const PATTERN_FLOWORK_HITL: &str = "cross_os/flowork-hitl-dag";
pub const PATTERN_ANTHROPIC_SKILL_MD: &str = "cross_os/anthropic-skill-md";

pub fn documented_patterns() -> &'static [(&'static str, &'static str)] {
    &[
        (PATTERN_ASSISTD_DAEMON, "IPC daemon + capability bridge"),
        (PATTERN_FLOWORK_HITL, "Workflow steps with human confirm"),
        (PATTERN_ANTHROPIC_SKILL_MD, "SKILL.md schema + tool membrane"),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossOsPeer {
    pub pattern_id: &'static str,
    pub label: &'static str,
}

/// Lista fixa de padrões Cross-OS — não scan de rede (AWAITING mesh federation).
pub fn discover_peers_stub() -> Vec<CrossOsPeer> {
    documented_patterns()
        .iter()
        .map(|(id, label)| CrossOsPeer {
            pattern_id: id,
            label,
        })
        .collect()
}
