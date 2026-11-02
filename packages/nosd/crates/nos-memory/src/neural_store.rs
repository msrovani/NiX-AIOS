//! Backend neural-sgdb real (`Sgdb` + `FileStorage`).

use crate::{GovError, Hit, MemoryRecord};
use base64::Engine as _;
use neural_sgdb::{demo_embed, FileStorage, MemoryLayer, RememberOptions, Sgdb, DOCTRINE};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use tracing::info;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub struct NeuralStore {
    db: Mutex<Sgdb>,
    root: PathBuf,
}

impl NeuralStore {
    pub fn open(root: &Path) -> Result<Self, GovError> {
        std::fs::create_dir_all(root).map_err(|e| GovError::Io(e.to_string()))?;
        let path = root.join("store");
        let fs = FileStorage::open(&path).map_err(|e| GovError::Io(e.to_string()))?;
        let mut db = Sgdb::open(fs).map_err(|e| GovError::Store(e.to_string()))?;
        let emb = demo_embed(DOCTRINE);
        let seeded = db
            .ensure_doctrine(&emb)
            .map_err(|e| GovError::Store(e.to_string()))?;
        if seeded {
            info!(target: "nosd", "nsgdb doctrine seeded path={}", path.display());
        }
        Ok(Self {
            db: Mutex::new(db),
            root: root.to_path_buf(),
        })
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn remember(&self, rec: &MemoryRecord) -> Result<String, GovError> {
        let mut db = self.db.lock();
        let ents: Vec<&str> = rec.entities.iter().map(|s| s.as_str()).collect();
        let opts = RememberOptions {
            scope: if rec.scope.is_empty() {
                None
            } else {
                Some(rec.scope.as_str())
            },
            entities: &ents,
            content_type: Some("text"),
            scope_dims: None,
            model_id: None,
            index_key: true,
        };
        let key = if rec.key.is_empty() {
            format!("aios/{}", uuid::Uuid::new_v4())
        } else {
            rec.key
                .strip_prefix("md/L3/")
                .or_else(|| rec.key.strip_prefix("md/L4/"))
                .or_else(|| rec.key.strip_prefix("md/L2/"))
                .unwrap_or(rec.key.as_str())
                .to_string()
        };
        let out = db
            .remember_text_with(&key, &rec.text, opts)
            .map_err(|e| GovError::Store(e.to_string()))?;
        Ok(out.storage_key)
    }

    pub fn remember_episodic(
        &self,
        user: &str,
        response: &str,
    ) -> Result<(String, String), GovError> {
        let mut db = self.db.lock();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        db.remember_episodic(user, response, now)
            .map_err(|e| GovError::Store(e.to_string()))
    }

    pub fn recall_lexical(
        &self,
        query: &str,
        k: usize,
        scope: Option<&str>,
    ) -> Result<Vec<Hit>, GovError> {
        let mut db = self.db.lock();
        let raw = match scope {
            Some(s) if !s.is_empty() => db
                .recall_lexical_scoped(query, k.max(1), s)
                .map_err(|e| GovError::Store(e.to_string()))?,
            _ => db
                .recall_lexical(query, k.max(1))
                .map_err(|e| GovError::Store(e.to_string()))?,
        };
        Ok(raw.into_iter().map(map_hit).collect())
    }

    pub fn recall_for_path(&self, path: &str, k: usize) -> Result<Vec<Hit>, GovError> {
        let mut db = self.db.lock();
        let raw = db
            .recall_for_path(path, k.max(1))
            .map_err(|e| GovError::Store(e.to_string()))?;
        Ok(raw.into_iter().map(map_hit).collect())
    }

    pub fn health(&self) -> bool {
        self.root.exists()
    }

    pub fn health_view(&self, view: &str) -> Result<serde_json::Value, GovError> {
        let mut db = self.db.lock();
        match view {
            "era" => {
                let models = db.indexed_model_ids();
                Ok(serde_json::json!({
                    "view": "era",
                    "indexed_models": models,
                    "embedder_note": "demo_trigram_not_cosine",
                }))
            }
            "staleness" => Ok(serde_json::json!({
                "view": "staleness",
                "ok": true,
                "note": "host_decides_curate"
            })),
            "tensions" => {
                let issues = db.scope_issues().unwrap_or_default();
                Ok(serde_json::json!({
                    "view": "tensions",
                    "scope_issues": issues.iter().map(|(s,r)| serde_json::json!({"scope": s, "reason": r})).collect::<Vec<_>>(),
                }))
            }
            _ => Err(GovError::Unsupported("health view")),
        }
    }

    pub fn curate(&self, op: &str, key: &str) -> Result<serde_json::Value, GovError> {
        let mut db = self.db.lock();
        match op {
            "forget" => {
                db.forget(key).map_err(|e| GovError::Store(e.to_string()))?;
                Ok(serde_json::json!({ "ok": true, "op": "forget", "key": key }))
            }
            "expire_old" | "decay" | "consolidate_recurrences" => Ok(serde_json::json!({
                "ok": true,
                "op": op,
                "key": key,
                "note": "scheduled_host_side"
            })),
            _ => Err(GovError::Unsupported("curate op")),
        }
    }

    pub fn record_count_approx(&self) -> Result<usize, GovError> {
        let hits = self.recall_lexical("", 64, None)?;
        Ok(hits.len())
    }

    /// Export NMD1 records (L2/L3 cognitivos) para telepathy mesh.
    pub fn export_records_b64(&self, max: usize) -> Result<Vec<String>, GovError> {
        let mut db = self.db.lock();
        let mut out = Vec::new();
        let max = max.clamp(1, 256);
        for layer in [MemoryLayer::L2EpisodicShort, MemoryLayer::L3EpisodicLong] {
            let prefix = format!("md/{}/", layer.as_str());
            let keys = db
                .scan_prefix(&prefix)
                .map_err(|e| GovError::Store(e.to_string()))?;
            for (sk, _) in keys {
                if out.len() >= max {
                    break;
                }
                // Doctrine seed não viaja (cada nó tem a sua).
                if sk.contains("nsgdb/doctrine") {
                    continue;
                }
                if let Ok(Some(rec)) = db.export_record(&sk) {
                    out.push(B64.encode(rec.encode()));
                }
            }
            if out.len() >= max {
                break;
            }
        }
        Ok(out)
    }

    /// Import records remotos via `merge_remote` (conflitos preservados).
    pub fn import_records_b64(&self, items: &[String]) -> Result<(usize, usize, usize), GovError> {
        use neural_sgdb::{MemoryRecord as NsgRec, MergeVerdict};
        let mut db = self.db.lock();
        let mut applied = 0usize;
        let mut conflict = 0usize;
        let mut rejected = 0usize;
        for b64 in items {
            let bytes = B64
                .decode(b64.trim())
                .map_err(|e| GovError::Store(format!("b64: {e}")))?;
            let rec = NsgRec::decode(&bytes).map_err(|e| GovError::Store(e.into()))?;
            match db
                .merge_remote(rec)
                .map_err(|e| GovError::Store(e.to_string()))?
            {
                MergeVerdict::Applied | MergeVerdict::Duplicate => applied += 1,
                MergeVerdict::Conflict => conflict += 1,
                MergeVerdict::Rejected
                | MergeVerdict::Stale
                | MergeVerdict::SelfPacket
                | MergeVerdict::RejectedByAuthority => rejected += 1,
            }
        }
        Ok((applied, conflict, rejected))
    }
}

fn map_hit(h: neural_sgdb::Hit) -> Hit {
    let (scope, entities) = h
        .provenance
        .as_ref()
        .map(|p| (p.scope.clone(), p.entities.clone()))
        .unwrap_or_default();
    let rel = h.rel.clone().unwrap_or_else(|| h.key.clone());
    Hit {
        key: h.key,
        rel,
        text: h.text,
        score: h.score,
        matched_terms: h.matched_terms,
        scope,
        entities,
        mode_used: "lexical".into(),
        basis: None,
    }
}
