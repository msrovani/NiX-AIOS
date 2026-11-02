//! MemoryGovernor — único gateway de memória cognitiva (doutrina neural-sgdb).
//!
//! Hermes/Cortex/RPC **não** tocam `Sgdb`/`FileStorage` directamente.

use crate::{GovError, MemoryRecord, NosMemory, RecallMode, RecallReq, RememberKind};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Token de turno: two-pass (recall antes de remember de utilizador).
#[derive(Debug, Clone)]
pub struct TurnToken {
    id: u64,
    recall_done: bool,
}

impl TurnToken {
    pub fn recall_done(&self) -> bool {
        self.recall_done
    }
}

static TURN_SEQ: AtomicU64 = AtomicU64::new(1);

impl NosMemory {
    /// Inicia turno cognitivo (chat).
    pub fn begin_turn(&self) -> TurnToken {
        TurnToken {
            id: TURN_SEQ.fetch_add(1, Ordering::Relaxed),
            recall_done: false,
        }
    }

    /// Recall governado — marca `recall_done` no token.
    pub fn recall_gov(
        &self,
        token: &mut TurnToken,
        req: RecallReq<'_>,
    ) -> Result<Vec<crate::Hit>, GovError> {
        let hits = self.recall_governed(req)?;
        token.recall_done = true;
        let _ = token.id;
        Ok(hits)
    }

    /// Remember de utilizador: exige two-pass neste turno.
    pub fn remember_user(&self, token: &TurnToken, rec: MemoryRecord) -> Result<String, GovError> {
        if !token.recall_done {
            return Err(GovError::TwoPassRequired);
        }
        self.remember_governed(RememberKind::User, rec)
    }

    /// Remember de sistema (OPAR/doctor/HITL outcome): scope+entities obrigatórios.
    pub fn remember_system(&self, rec: MemoryRecord) -> Result<String, GovError> {
        self.remember_governed(RememberKind::System, rec)
    }

    /// Episódio verbatim L2.
    pub fn remember_episodic_gov(
        &self,
        token: &TurnToken,
        user: &str,
        response: &str,
    ) -> Result<(String, String), GovError> {
        if !token.recall_done {
            return Err(GovError::TwoPassRequired);
        }
        self.remember_episodic_inner(user, response)
    }
}

/// Validação pura (testável sem abrir DB).
pub fn validate_recall_mode(
    mode: RecallMode,
    has_embedding: bool,
    embedder_real: bool,
) -> RecallMode {
    match mode {
        RecallMode::Semantic if !has_embedding => RecallMode::Lexical,
        RecallMode::Hybrid if !embedder_real || !has_embedding => RecallMode::Lexical,
        other => other,
    }
}

pub fn validate_followup_key(key: &str) -> Result<(), GovError> {
    if key.starts_with("md/L") && key.contains('/') && key.len() > 8 {
        Ok(())
    } else {
        Err(GovError::IncompleteStorageKey)
    }
}

pub fn validate_system_record(rec: &MemoryRecord) -> Result<(), GovError> {
    // Scope obrigatório: vazio = GLOBAL e entra em recall null-scope
    // (filter_scope(None) só aceita scope vazio) — porta de memory poisoning.
    if rec.scope.trim().is_empty() {
        return Err(GovError::ScopeRequired);
    }
    if rec.entities.is_empty() {
        return Err(GovError::EntitiesRequired);
    }
    Ok(())
}

/// Single-flight writer lock partilhado (um open lógico por processo).
pub type WriterLock = Arc<Mutex<()>>;

pub fn new_writer_lock() -> WriterLock {
    Arc::new(Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryBackendKind, MemoryRecord, RecallMode, RecallReq};
    use tempfile::tempdir;

    #[test]
    fn two_pass_blocks_user_remember() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::NsgdbLegacy).unwrap();
        let tok = m.begin_turn();
        let err = m
            .remember_user(
                &tok,
                MemoryRecord {
                    key: "k1".into(),
                    text: "x".into(),
                    scope: "aios/user".into(),
                    entities: vec!["mom/fact".into()],
                },
            )
            .unwrap_err();
        assert!(matches!(err, GovError::TwoPassRequired));
    }

    #[test]
    fn two_pass_allows_after_recall() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::NsgdbLegacy).unwrap();
        let mut tok = m.begin_turn();
        let _ = m
            .recall_gov(
                &mut tok,
                RecallReq {
                    query: "nada",
                    k: 3,
                    scope: Some("aios/user"),
                    entities: &[],
                    mode: RecallMode::Lexical,
                    embedding: None,
                    path: None,
                },
            )
            .unwrap();
        assert!(tok.recall_done());
        m.remember_user(
            &tok,
            MemoryRecord {
                key: "md/L3/t".into(),
                text: "prefiro pt".into(),
                scope: "aios/user".into(),
                entities: vec!["mom/pref".into()],
            },
        )
        .unwrap();
    }

    #[test]
    fn null_scope_hides_scoped() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::NsgdbLegacy).unwrap();
        m.remember_system(MemoryRecord {
            key: "md/L3/secret".into(),
            text: "wifi lab ssid secreto".into(),
            scope: "aios/user".into(),
            entities: vec!["mom/fact".into()],
        })
        .unwrap();
        let global = m
            .recall_governed(RecallReq {
                query: "wifi",
                k: 5,
                scope: None,
                entities: &[],
                mode: RecallMode::Lexical,
                embedding: None,
                path: None,
            })
            .unwrap();
        assert!(
            global
                .iter()
                .all(|h| h.scope.is_empty() || h.scope != "aios/user"),
            "null-scope nao deve ver aios/user: {global:?}"
        );
        let scoped = m
            .recall_governed(RecallReq {
                query: "wifi",
                k: 5,
                scope: Some("aios/user"),
                entities: &[],
                mode: RecallMode::Lexical,
                embedding: None,
                path: None,
            })
            .unwrap();
        assert!(!scoped.is_empty(), "scoped deve ver o facto");
    }

    #[test]
    fn semantic_without_embedding_downgrades() {
        assert_eq!(
            validate_recall_mode(RecallMode::Semantic, false, true),
            RecallMode::Lexical
        );
        assert_eq!(
            validate_recall_mode(RecallMode::Hybrid, true, false),
            RecallMode::Lexical
        );
    }

    #[test]
    fn followup_key_must_be_full() {
        assert!(validate_followup_key("md/L4/nsgdb/doctrine").is_ok());
        assert!(validate_followup_key("doctrine").is_err());
    }

    #[test]
    fn soul_markers_present() {
        let soul = include_str!("../../nos-hermes/soul/SOUL.md");
        for m in [
            "NSGDB_DOCTRINE_V1",
            "TWO_PASS",
            "NULL_SCOPE",
            "ADD_ONLY",
            "HIT_NOT_ORDER",
        ] {
            assert!(soul.contains(m), "SOUL falta marcador {m}");
        }
    }

    #[test]
    fn system_requires_scope() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::NsgdbLegacy).unwrap();
        let err = m
            .remember_system(MemoryRecord {
                key: "md/L3/global".into(),
                text: "x".into(),
                scope: "  ".into(),
                entities: vec!["mom/fact".into()],
            })
            .unwrap_err();
        assert!(matches!(err, GovError::ScopeRequired));
    }

    #[test]
    fn system_requires_entities() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::NsgdbLegacy).unwrap();
        let err = m
            .remember_system(MemoryRecord {
                key: "k".into(),
                text: "x".into(),
                scope: "aios/node".into(),
                entities: vec![],
            })
            .unwrap_err();
        assert!(matches!(err, GovError::EntitiesRequired));
    }
}
