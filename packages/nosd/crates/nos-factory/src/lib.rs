//! App Factory path A — validate wasmi + CapGate + promote (ADR-0059).
//! Artifact MD frontmatter gate (ADR-0052 subset) — deny-by-default sem schema/kind/name/goal/acionaveis.
//! Seek-before-generate (F.3) + federated PackageStore.

mod seek;
mod skill_opt;
mod store;

pub use seek::{
    seek_audit_line, seek_skill, CandidateSource, SeekConfig, SeekOutcome, SeekReport,
    SeekSourceTried, SkillCandidate, SkillNeed,
};
pub use skill_opt::{plan_skill_opt, self_evolve_agent_md, SkillOptAction, SkillOptPlan};
pub use store::{
    bytes_to_hex, fnv1a64, fnv1a64_hex16, FederatedIndex, FederatedIndexEntry, ImportMeta,
    ImportProvenance, PackageStore, SandboxStatus, StoreError, StoredPackage, FEDERATED_INDEX_CAP,
    PACKAGE_STORE_CAP,
};

use nos_core::CapBits;
use nos_wasm::{tiny_identity_wasm, ExecReport, FrugalRuntime, Grants, Inspection, WasmError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum FactoryError {
    #[error("path B/C EngineAbsent until Linux isolation VERIFIED")]
    PathBcAbsent,
    #[error("wasm: {0}")]
    Wasm(#[from] WasmError),
    #[error("promote denied")]
    PromoteDenied,
    #[error("artifact md: {0}")]
    ArtifactMd(&'static str),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FactoryPath {
    AWasmi,
    BCranelift,
    CNative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDraft {
    pub name: String,
    pub wasm: Vec<u8>,
    pub path: FactoryPath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotedArtifact {
    pub name: String,
    /// BLAKE3 hex (64) — commitment de conteúdo (S1), não o FNV de dedup.
    pub content_id: String,
    pub bytes: usize,
    /// ABI detetada: 1 = só `run`; 2 = `run` + `result_ptr`/`result_len`.
    pub abi: u32,
    pub fuel_used: u64,
    /// `true` quando o draft veio vazio e a fábrica usou o módulo identidade.
    /// É um placeholder honesto (sem backend de codegen), não uma app gerada.
    pub placeholder: bool,
}

/// Commitment BLAKE3 do módulo (mesma noção usada por `skill.install`).
pub fn content_id(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub struct AppFactory {
    rt: FrugalRuntime,
}

impl Default for AppFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl AppFactory {
    pub fn new() -> Self {
        Self {
            rt: FrugalRuntime::new(),
        }
    }

    /// Inspeção estática: imports/ABI sem executar `start`.
    pub fn inspect(&self, wasm: &[u8]) -> Result<Inspection, FactoryError> {
        Ok(self.rt.inspect(wasm)?)
    }

    /// Draft vazio usa o módulo identidade (placeholder declarado no
    /// `PromotedArtifact`); não fingimos um backend de síntese.
    fn code_for(&self, draft: &ArtifactDraft) -> (Vec<u8>, bool) {
        if draft.wasm.is_empty() {
            (tiny_identity_wasm(), true)
        } else {
            (draft.wasm.clone(), false)
        }
    }

    pub fn validate(
        &self,
        draft: &ArtifactDraft,
        caps: CapBits,
        gen: u64,
    ) -> Result<ExecReport, FactoryError> {
        match draft.path {
            FactoryPath::AWasmi => {
                let (code, _) = self.code_for(draft);
                Ok(self.rt.execute(&code, Grants::caps(caps), gen, 7)?)
            }
            FactoryPath::BCranelift | FactoryPath::CNative => Err(FactoryError::PathBcAbsent),
        }
    }

    pub fn promote(
        &self,
        draft: &ArtifactDraft,
        caps: CapBits,
        gen: u64,
        allow_promote: bool,
    ) -> Result<PromotedArtifact, FactoryError> {
        if !allow_promote {
            return Err(FactoryError::PromoteDenied);
        }
        let report = self.validate(draft, caps, gen)?;
        let (owned, placeholder) = self.code_for(draft);
        let id = content_id(&owned);
        info!(
            target: "nosd",
            "factory promote name={} content_id={} abi={} placeholder={}",
            draft.name,
            id,
            report.abi,
            placeholder
        );
        Ok(PromotedArtifact {
            name: draft.name.clone(),
            content_id: id,
            bytes: owned.len(),
            abi: report.abi,
            fuel_used: report.fuel_used,
            placeholder,
        })
    }
}

/// Validação mínima ADR-0052: frontmatter YAML entre `---` com campos obrigatórios.
/// `content_hash` / Ed25519 = residual (host pode checar depois); aqui = deny sem contrato.
pub fn validate_artifact_md(text: &str) -> Result<ArtifactMdMeta, FactoryError> {
    let t = text.trim_start();
    if !t.starts_with("---") {
        return Err(FactoryError::ArtifactMd("missing frontmatter"));
    }
    let rest = &t[3..];
    let end = rest
        .find("\n---")
        .ok_or(FactoryError::ArtifactMd("unclosed frontmatter"))?;
    let fm = &rest[..end];
    let body = rest[end + 4..].trim_start();

    let schema = fm_line(fm, "schema");
    let kind = fm_line(fm, "kind");
    let name = fm_line(fm, "name");
    let goal = fm_line(fm, "goal");
    let contexto = fm_line(fm, "contexto");
    let acionaveis = fm_line(fm, "acionaveis");

    if schema != Some("1") {
        return Err(FactoryError::ArtifactMd("schema!=1"));
    }
    let kind = kind.ok_or(FactoryError::ArtifactMd("kind"))?;
    let name = name.ok_or(FactoryError::ArtifactMd("name"))?;
    if goal.is_none() {
        return Err(FactoryError::ArtifactMd("goal"));
    }
    if contexto.is_none() {
        return Err(FactoryError::ArtifactMd("contexto"));
    }
    if acionaveis.is_none() {
        return Err(FactoryError::ArtifactMd("acionaveis"));
    }
    // Seções Markdown mínimas
    for sec in [
        "## Contexto",
        "## Goal",
        "## Acionaveis",
        "## Workflow",
        "## Pre-Flight",
        "## Success Criteria",
        "## Failure Policy",
    ] {
        if !body.contains(sec) {
            return Err(FactoryError::ArtifactMd("missing section"));
        }
    }
    Ok(ArtifactMdMeta {
        kind: kind.to_string(),
        name: name.to_string(),
        body_fnv: store::fnv1a64(body.as_bytes()),
    })
}

fn fm_line<'a>(fm: &'a str, key: &str) -> Option<&'a str> {
    for line in fm.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix(':') {
                let v = v.trim().trim_matches('"');
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMdMeta {
    pub kind: String,
    pub name: String,
    pub body_fnv: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_a_ok() {
        let f = AppFactory::new();
        let d = ArtifactDraft {
            name: "echo".into(),
            wasm: vec![],
            path: FactoryPath::AWasmi,
        };
        let r = f.validate(&d, CapBits::FULL, 1).unwrap();
        assert_eq!(r.value, 7);
        assert_eq!(r.abi, 1);
        let p = f.promote(&d, CapBits::FULL, 1, true).unwrap();
        assert_eq!(p.content_id.len(), 64, "BLAKE3, não FNV");
        assert!(p.placeholder, "draft vazio = identidade declarada");
        assert_eq!(p.bytes, tiny_identity_wasm().len());
    }

    #[test]
    fn app_abi_v2_promotes_without_placeholder() {
        let f = AppFactory::new();
        let code = nos_wasm::echo_app_wasm();
        let i = f.inspect(&code).unwrap();
        assert_eq!(i.abi, 2);
        assert!(i.compatible(), "{i:?}");
        let d = ArtifactDraft {
            name: "echo-app".into(),
            wasm: code.clone(),
            path: FactoryPath::AWasmi,
        };
        let r = f.validate(&d, CapBits::FULL, 1).unwrap();
        assert_eq!(r.value, 7);
        assert_eq!(
            r.result_text().as_deref(),
            Some("{\"app\":\"echo\"}"),
            "canal de resultado ABI v2"
        );
        let p = f.promote(&d, CapBits::FULL, 1, true).unwrap();
        assert!(!p.placeholder);
        assert_eq!(p.abi, 2);
        assert_eq!(p.content_id, content_id(&code));
    }

    #[test]
    fn unknown_import_is_denied_at_inspection() {
        let f = AppFactory::new();
        let code = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "fd_write"
                    (func (param i32 i32 i32 i32) (result i32)))
                (func (export "run") (param i32) (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        let i = f.inspect(&code).unwrap();
        assert!(!i.compatible());
        let d = ArtifactDraft {
            name: "wasi".into(),
            wasm: code,
            path: FactoryPath::AWasmi,
        };
        let e = f.validate(&d, CapBits::FULL, 1).unwrap_err();
        assert!(
            matches!(e, FactoryError::Wasm(WasmError::ImportDenied(_))),
            "{e:?}"
        );
    }

    #[test]
    fn path_b_absent() {
        let f = AppFactory::new();
        let d = ArtifactDraft {
            name: "x".into(),
            wasm: vec![],
            path: FactoryPath::BCranelift,
        };
        assert!(matches!(
            f.validate(&d, CapBits::FULL, 1),
            Err(FactoryError::PathBcAbsent)
        ));
    }

    #[test]
    fn artifact_md_ok() {
        let md = r#"---
schema: 1
kind: skill
name: echo
goal: reply
contexto: test
acionaveis: [oneshot]
---
## Contexto
x
## Goal
y
## Acionaveis
z
## Workflow
w
## Pre-Flight
p
## Success Criteria
s
## Failure Policy
f
"#;
        let m = validate_artifact_md(md).unwrap();
        assert_eq!(m.kind, "skill");
        assert_eq!(m.name, "echo");
    }

    #[test]
    fn artifact_md_deny() {
        assert!(validate_artifact_md("no fm").is_err());
    }
}
