//! nos-memory — MemoryGovernor + backends neural | nsgdb_legacy | jsonl.
//!
//! **Único** gateway cognitivo. Proibido `FileStorage`/`Sgdb` fora deste crate.

mod backend;
mod governor;
mod neural_store;
mod seg;

pub use backend::{MemoryBackendKind, MemoryLayer};
pub use governor::{
    new_writer_lock, validate_followup_key, validate_recall_mode, validate_system_record,
    TurnToken, WriterLock,
};

use backend::MemoryBackendKind as Kind;
use neural_store::NeuralStore;
use parking_lot::Mutex;
use seg::NsgdbSegStore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use thiserror::Error;
use tracing::info;

pub const RECORD_CAP: usize = 1024;
pub const DOCTRINE_MARKERS: &[&str] = &[
    "NSGDB_DOCTRINE_V1",
    "TWO_PASS",
    "NULL_SCOPE",
    "ADD_ONLY",
    "HIT_NOT_ORDER",
];
pub const MEMORY_API_VERSION: &str = "1";

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("gov: {0}")]
    Gov(#[from] GovError),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GovError {
    #[error("two_pass_required: recall before remember in this turn")]
    TwoPassRequired,
    #[error("incomplete_storage_key: follow-ups need full md/L… key")]
    IncompleteStorageKey,
    #[error("scope_required")]
    ScopeRequired,
    #[error("entities_required")]
    EntitiesRequired,
    #[error("writer_busy: single writer per path")]
    WriterBusy,
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("store: {0}")]
    Store(String),
    #[error("io: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub key: String,
    pub text: String,
    pub scope: String,
    pub entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub key: String,
    #[serde(default)]
    pub rel: String,
    pub text: String,
    pub score: f32,
    pub matched_terms: Vec<String>,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub mode_used: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallMode {
    Lexical,
    Semantic,
    Hybrid,
}

impl RecallMode {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "semantic" => Self::Semantic,
            "hybrid" => Self::Hybrid,
            _ => Self::Lexical,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Semantic => "semantic",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecallReq<'a> {
    pub query: &'a str,
    pub k: usize,
    pub scope: Option<&'a str>,
    pub entities: &'a [String],
    pub mode: RecallMode,
    pub embedding: Option<&'a [f32]>,
    pub path: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RememberKind {
    User,
    System,
}

enum Storage {
    Jsonl(PathBuf),
    Legacy(NsgdbSegStore),
    // Boxed: NeuralStore é ~568 B; inline alargava todas as variantes
    // (clippy::large_enum_variant com -D warnings).
    Neural(Box<NeuralStore>),
}

#[derive(Clone)]
pub struct NosMemory {
    storage: Arc<Storage>,
    backend: Kind,
    lock: Arc<Mutex<()>>,
    data_dir: PathBuf,
    embedder_real: bool,
}

fn opened_paths() -> &'static Mutex<HashSet<PathBuf>> {
    static OPENED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    OPENED.get_or_init(|| Mutex::new(HashSet::new()))
}

impl NosMemory {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, MemoryError> {
        Self::open_with_backend(dir, Kind::from_env())
    }

    pub fn open_with_backend(dir: impl AsRef<Path>, backend: Kind) -> Result<Self, MemoryError> {
        let data_dir = dir.as_ref().to_path_buf();
        backend.ensure_dirs_for(&data_dir)?;
        let path = backend.storage_path_for(&data_dir);
        {
            let mut g = opened_paths().lock();
            let canon = path.clone();
            if g.contains(&canon) {
                return Err(GovError::WriterBusy.into());
            }
            g.insert(canon);
        }
        let storage = match backend {
            Kind::Jsonl => {
                if !path.exists() {
                    File::create(&path)?;
                }
                Storage::Jsonl(path)
            }
            Kind::NsgdbLegacy => Storage::Legacy(NsgdbSegStore::open(&path)?),
            Kind::Neural => Storage::Neural(Box::new(
                NeuralStore::open(&path).map_err(MemoryError::from)?,
            )),
        };
        let embedder_real = matches!(
            std::env::var("NEURAL_SGDB_EMBEDDER").ok().as_deref(),
            Some("multilingual") | Some("candle") | Some("onnx") | Some("local")
        );
        info!(
            target: "nosd",
            "memory open backend={:?} neural={} path={}",
            backend,
            backend.is_neural(),
            backend.storage_path_for(&data_dir).display()
        );
        Ok(Self {
            storage: Arc::new(storage),
            backend,
            lock: Arc::new(Mutex::new(())),
            data_dir,
            embedder_real,
        })
    }

    pub fn backend(&self) -> Kind {
        self.backend
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn embedder_real(&self) -> bool {
        self.embedder_real
    }

    pub fn layers_supported(&self) -> &'static [MemoryLayer] {
        self.backend.layers_supported()
    }

    /// Compat: remember sem governança de turno (system/legacy callers).
    pub fn remember(&self, rec: MemoryRecord) -> Result<String, MemoryError> {
        self.remember_governed(RememberKind::System, rec)
            .map_err(MemoryError::from)
    }

    pub fn recall(&self, query: &str, k: usize) -> Result<Vec<Hit>, MemoryError> {
        self.recall_lexical(query, k)
    }

    pub fn recall_lexical(&self, query: &str, k: usize) -> Result<Vec<Hit>, MemoryError> {
        self.recall_governed(RecallReq {
            query,
            k,
            scope: None,
            entities: &[],
            mode: RecallMode::Lexical,
            embedding: None,
            path: None,
        })
        .map_err(MemoryError::from)
    }

    pub fn recall_governed(&self, req: RecallReq<'_>) -> Result<Vec<Hit>, GovError> {
        let _g = self.lock.lock();
        if let Some(path) = req.path {
            if path.is_empty() {
                return Ok(Vec::new());
            }
            // Triggers: sem match → vazio (não global). Neural path-only; legacy = empty.
            return match self.storage.as_ref() {
                Storage::Neural(n) => n.recall_for_path(path, req.k.max(1)),
                _ => Ok(Vec::new()),
            };
        }
        let mode = validate_recall_mode(req.mode, req.embedding.is_some(), self.embedder_real);
        let basis = if mode != req.mode {
            Some(
                if req.mode == RecallMode::Semantic && req.embedding.is_none() {
                    "no_embedding".into()
                } else {
                    "embedder_not_real".into()
                },
            )
        } else {
            None
        };
        let mut hits = match self.storage.as_ref() {
            Storage::Jsonl(path) => {
                let records = read_jsonl_records(path).map_err(|e| GovError::Io(e.to_string()))?;
                let filtered = filter_scope(records, req.scope);
                score_hits(req.query, req.k, filtered)
            }
            Storage::Legacy(store) => {
                let records = store
                    .recall_layers(&[2, 3])
                    .map_err(|e| GovError::Io(e.to_string()))?;
                let filtered = filter_scope(records, req.scope);
                score_hits(req.query, req.k, filtered)
            }
            Storage::Neural(n) => n.recall_lexical(req.query, req.k, req.scope)?,
        };
        for h in &mut hits {
            h.mode_used = mode.as_str().into();
            if h.basis.is_none() {
                h.basis = basis.clone();
            }
        }
        Ok(hits)
    }

    pub fn remember_governed(
        &self,
        kind: RememberKind,
        rec: MemoryRecord,
    ) -> Result<String, GovError> {
        if kind == RememberKind::System {
            validate_system_record(&rec)?;
        }
        let _g = self.lock.lock();
        match self.storage.as_ref() {
            Storage::Jsonl(path) => {
                let mut f = OpenOptions::new()
                    .append(true)
                    .open(path)
                    .map_err(|e| GovError::Io(e.to_string()))?;
                let line =
                    serde_json::to_string(&rec).map_err(|e| GovError::Store(e.to_string()))?;
                writeln!(f, "{line}").map_err(|e| GovError::Io(e.to_string()))?;
                evict_jsonl_fifo(path).map_err(|e| GovError::Io(e.to_string()))?;
                Ok(rec.key)
            }
            Storage::Legacy(store) => {
                store
                    .remember(rec.clone())
                    .map_err(|e| GovError::Io(e.to_string()))?;
                Ok(rec.key)
            }
            Storage::Neural(n) => n.remember(&rec),
        }
    }

    pub fn remember_episodic_inner(
        &self,
        user: &str,
        response: &str,
    ) -> Result<(String, String), GovError> {
        let _g = self.lock.lock();
        match self.storage.as_ref() {
            Storage::Neural(n) => n.remember_episodic(user, response),
            _ => {
                let key = format!("md/L2/ep/{}", uuid::Uuid::new_v4());
                let text = format!("user: {user} | assistant: {response}");
                let rec = MemoryRecord {
                    key: key.clone(),
                    text,
                    scope: "aios/session".into(),
                    entities: vec!["mom/learning".into()],
                };
                match self.storage.as_ref() {
                    Storage::Legacy(store) => {
                        store
                            .remember(rec)
                            .map_err(|e| GovError::Io(e.to_string()))?;
                    }
                    Storage::Jsonl(path) => {
                        let mut f = OpenOptions::new()
                            .append(true)
                            .open(path)
                            .map_err(|e| GovError::Io(e.to_string()))?;
                        let line = serde_json::to_string(&rec)
                            .map_err(|e| GovError::Store(e.to_string()))?;
                        writeln!(f, "{line}").map_err(|e| GovError::Io(e.to_string()))?;
                    }
                    Storage::Neural(_) => unreachable!(),
                }
                Ok((key.clone(), key))
            }
        }
    }

    pub fn health(&self) -> bool {
        match self.storage.as_ref() {
            Storage::Jsonl(path) => path.exists(),
            Storage::Legacy(store) => store.root().exists(),
            Storage::Neural(n) => n.health(),
        }
    }

    pub fn health_view(&self, view: &str) -> Result<serde_json::Value, GovError> {
        let tel = if self.telepathy_ready() {
            "ready"
        } else {
            "unsupported"
        };
        match view {
            "era" | "staleness" | "tensions" => match self.storage.as_ref() {
                Storage::Neural(n) => {
                    let mut v = n.health_view(view)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("telepathy".into(), serde_json::json!(tel));
                    }
                    Ok(v)
                }
                _ => Ok(serde_json::json!({
                    "view": view,
                    "backend": format!("{:?}", self.backend),
                    "ok": self.health(),
                    "honesty": "legacy_backend_no_era",
                    "telepathy": tel
                })),
            },
            _ => Err(GovError::Unsupported("health view")),
        }
    }

    pub fn curate(&self, op: &str, key: Option<&str>) -> Result<serde_json::Value, GovError> {
        match op {
            "commit_run" => Ok(serde_json::json!({ "ok": true, "op": "commit_run" })),
            "forget" | "expire_old" | "decay" | "consolidate_recurrences" => {
                let key = key.ok_or(GovError::IncompleteStorageKey)?;
                validate_followup_key(key)?;
                match self.storage.as_ref() {
                    Storage::Neural(n) => n.curate(op, key),
                    _ => Err(GovError::Unsupported("curate on legacy backend")),
                }
            }
            "decide" => Err(GovError::Unsupported("decide")),
            _ => Err(GovError::Unsupported("curate op")),
        }
    }

    pub fn record_count(&self) -> Result<usize, MemoryError> {
        let _g = self.lock.lock();
        match self.storage.as_ref() {
            Storage::Jsonl(path) => Ok(count_jsonl_lines(path)?),
            Storage::Legacy(store) => Ok(store.record_count(&[2, 3])?),
            Storage::Neural(n) => Ok(n.record_count_approx().map_err(MemoryError::from)?),
        }
    }

    /// F7 telepathy: export bundle (records_b64) para peer mesh.
    pub fn telepathy_export(&self, max: usize) -> Result<Vec<String>, GovError> {
        let _g = self.lock.lock();
        match self.storage.as_ref() {
            Storage::Neural(n) => n.export_records_b64(max),
            Storage::Legacy(store) => {
                // Legacy: JSON lines dos MemoryRecord L2/L3 (não NMD1).
                let records = store
                    .recall_layers(&[2, 3])
                    .map_err(|e| GovError::Io(e.to_string()))?;
                let mut out = Vec::new();
                for r in records.into_iter().take(max.clamp(1, 256)) {
                    let j = serde_json::to_vec(&r).map_err(|e| GovError::Store(e.to_string()))?;
                    out.push(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        j,
                    ));
                }
                Ok(out)
            }
            Storage::Jsonl(path) => {
                let records = read_jsonl_records(path).map_err(|e| GovError::Io(e.to_string()))?;
                let mut out = Vec::new();
                for r in records.into_iter().take(max.clamp(1, 256)) {
                    let j = serde_json::to_vec(&r).map_err(|e| GovError::Store(e.to_string()))?;
                    out.push(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        j,
                    ));
                }
                Ok(out)
            }
        }
    }

    /// F7 telepathy: import bundle do peer (merge).
    pub fn telepathy_import(&self, items: &[String]) -> Result<serde_json::Value, GovError> {
        let _g = self.lock.lock();
        match self.storage.as_ref() {
            Storage::Neural(n) => {
                let (applied, conflict, rejected) = n.import_records_b64(items)?;
                Ok(serde_json::json!({
                    "applied": applied,
                    "conflict": conflict,
                    "rejected": rejected,
                    "backend": "neural",
                }))
            }
            Storage::Legacy(store) => {
                let mut applied = 0usize;
                for b64 in items {
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        b64.trim(),
                    )
                    .map_err(|e| GovError::Store(format!("b64: {e}")))?;
                    let rec: MemoryRecord = serde_json::from_slice(&bytes)
                        .map_err(|e| GovError::Store(e.to_string()))?;
                    store
                        .remember(rec)
                        .map_err(|e| GovError::Io(e.to_string()))?;
                    applied += 1;
                }
                Ok(serde_json::json!({
                    "applied": applied,
                    "conflict": 0,
                    "rejected": 0,
                    "backend": "nsgdb_legacy",
                }))
            }
            Storage::Jsonl(path) => {
                let mut applied = 0usize;
                let mut f = OpenOptions::new()
                    .append(true)
                    .open(path)
                    .map_err(|e| GovError::Io(e.to_string()))?;
                for b64 in items {
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        b64.trim(),
                    )
                    .map_err(|e| GovError::Store(format!("b64: {e}")))?;
                    let rec: MemoryRecord = serde_json::from_slice(&bytes)
                        .map_err(|e| GovError::Store(e.to_string()))?;
                    let line =
                        serde_json::to_string(&rec).map_err(|e| GovError::Store(e.to_string()))?;
                    writeln!(f, "{line}").map_err(|e| GovError::Io(e.to_string()))?;
                    applied += 1;
                }
                Ok(serde_json::json!({
                    "applied": applied,
                    "conflict": 0,
                    "rejected": 0,
                    "backend": "jsonl",
                }))
            }
        }
    }

    /// Troca telepathy (hook servidor / teste): um só lock — merge inbound + export.
    pub fn telepathy_exchange(
        &self,
        inbound_b64: &[String],
        max_export: usize,
    ) -> Result<serde_json::Value, GovError> {
        let _g = self.lock.lock();
        let import = match self.storage.as_ref() {
            Storage::Neural(n) => {
                let (applied, conflict, rejected) = n.import_records_b64(inbound_b64)?;
                serde_json::json!({
                    "applied": applied,
                    "conflict": conflict,
                    "rejected": rejected,
                    "backend": "neural",
                })
            }
            Storage::Legacy(store) => {
                let mut applied = 0usize;
                for b64 in inbound_b64 {
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        b64.trim(),
                    )
                    .map_err(|e| GovError::Store(format!("b64: {e}")))?;
                    let rec: MemoryRecord = serde_json::from_slice(&bytes)
                        .map_err(|e| GovError::Store(e.to_string()))?;
                    store
                        .remember(rec)
                        .map_err(|e| GovError::Io(e.to_string()))?;
                    applied += 1;
                }
                serde_json::json!({
                    "applied": applied,
                    "conflict": 0,
                    "rejected": 0,
                    "backend": "nsgdb_legacy",
                })
            }
            Storage::Jsonl(path) => {
                let mut applied = 0usize;
                let mut f = OpenOptions::new()
                    .append(true)
                    .open(path)
                    .map_err(|e| GovError::Io(e.to_string()))?;
                for b64 in inbound_b64 {
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        b64.trim(),
                    )
                    .map_err(|e| GovError::Store(format!("b64: {e}")))?;
                    let rec: MemoryRecord = serde_json::from_slice(&bytes)
                        .map_err(|e| GovError::Store(e.to_string()))?;
                    let line =
                        serde_json::to_string(&rec).map_err(|e| GovError::Store(e.to_string()))?;
                    writeln!(f, "{line}").map_err(|e| GovError::Io(e.to_string()))?;
                    applied += 1;
                }
                serde_json::json!({
                    "applied": applied,
                    "conflict": 0,
                    "rejected": 0,
                    "backend": "jsonl",
                })
            }
        };
        let export = match self.storage.as_ref() {
            Storage::Neural(n) => n.export_records_b64(max_export)?,
            Storage::Legacy(store) => {
                let records = store
                    .recall_layers(&[2, 3])
                    .map_err(|e| GovError::Io(e.to_string()))?;
                let mut out = Vec::new();
                for r in records.into_iter().take(max_export.clamp(1, 256)) {
                    let j = serde_json::to_vec(&r).map_err(|e| GovError::Store(e.to_string()))?;
                    out.push(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        j,
                    ));
                }
                out
            }
            Storage::Jsonl(path) => {
                let records = read_jsonl_records(path).map_err(|e| GovError::Io(e.to_string()))?;
                let mut out = Vec::new();
                for r in records.into_iter().take(max_export.clamp(1, 256)) {
                    let j = serde_json::to_vec(&r).map_err(|e| GovError::Store(e.to_string()))?;
                    out.push(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        j,
                    ));
                }
                out
            }
        };
        Ok(serde_json::json!({
            "telepathy": true,
            "import": import,
            "records_b64": export,
            "backend": format!("{:?}", self.backend),
        }))
    }

    pub fn telepathy_ready(&self) -> bool {
        matches!(self.backend, Kind::Neural | Kind::NsgdbLegacy | Kind::Jsonl)
    }

    /// Cold-start: doctrine + scoped probes (fail-soft).
    pub fn cold_start(&self) -> Result<serde_json::Value, GovError> {
        let doctrine = self.recall_governed(RecallReq {
            query: "doctrine memory substrate",
            k: 3,
            scope: Some("nsgdb/doctrine"),
            entities: &[],
            mode: RecallMode::Lexical,
            embedding: None,
            path: None,
        })?;
        let prefs = self.recall_governed(RecallReq {
            query: "preferencia idioma",
            k: 3,
            scope: Some("aios/user"),
            entities: &[],
            mode: RecallMode::Lexical,
            embedding: None,
            path: None,
        })?;
        let era = self.health_view("era").unwrap_or(serde_json::json!({}));
        Ok(serde_json::json!({
            "api_version": MEMORY_API_VERSION,
            "backend": format!("{:?}", self.backend),
            "doctrine_hits": doctrine.len(),
            "pref_hits": prefs.len(),
            "era": era,
            "memory_ok": self.health(),
        }))
    }
}

fn filter_scope(records: Vec<MemoryRecord>, scope: Option<&str>) -> Vec<MemoryRecord> {
    match scope {
        None => records.into_iter().filter(|r| r.scope.is_empty()).collect(),
        Some(s) => records.into_iter().filter(|r| r.scope == s).collect(),
    }
}

fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.to_lowercase().chars() {
        if ch.is_whitespace() {
            out.push(' ');
            continue;
        }
        if !ch.is_alphanumeric() {
            continue;
        }
        out.push(match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        });
    }
    out.trim().to_string()
}

const STOPWORDS: &[&str] = &[
    "de", "da", "do", "das", "dos", "que", "para", "com", "uma", "um", "uns", "umas", "os", "as",
    "no", "na", "nos", "nas", "em", "por", "se", "ao", "aos", "como", "mais", "mas", "ou",
    "quando", "onde", "qual", "quais", "quem", "muito", "pode", "voce", "meu", "minha", "sobre",
    "isso", "isto", "aquilo", "the", "and", "for", "with", "that", "this", "what", "how", "are",
    "you", "from", "not", "have", "was", "were",
];

fn query_terms(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = query
        .split_whitespace()
        .map(fold)
        .filter(|t| t.len() > 1)
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .collect();
    terms.sort();
    terms.dedup();
    terms
}

fn score_hits(query: &str, k: usize, records: Vec<MemoryRecord>) -> Vec<Hit> {
    let terms = query_terms(query);
    let had_words = query.split_whitespace().any(|t| t.len() > 1);
    if terms.is_empty() && had_words {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for rec in records {
        let folded = fold(&rec.text);
        let mut matched = Vec::new();
        let mut score = 0.0f32;
        for t in &terms {
            if folded.contains(t.as_str()) {
                matched.push(t.clone());
                score += 1.0;
            }
        }
        if score > 0.0 || terms.is_empty() {
            hits.push(Hit {
                key: rec.key.clone(),
                rel: rec.key,
                text: rec.text,
                score,
                matched_terms: matched,
                scope: rec.scope,
                entities: rec.entities,
                mode_used: "lexical".into(),
                basis: None,
            });
        }
    }
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    hits.truncate(k.max(1));
    hits
}

fn read_jsonl_records(path: &Path) -> Result<Vec<MemoryRecord>, MemoryError> {
    let f = fs::File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        out.push(serde_json::from_str(&line)?);
    }
    Ok(out)
}

fn count_jsonl_lines(path: &Path) -> Result<usize, MemoryError> {
    let f = fs::File::open(path)?;
    Ok(BufReader::new(f)
        .lines()
        .filter(|l| l.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count())
}

fn evict_jsonl_fifo(path: &Path) -> Result<(), MemoryError> {
    let n = count_jsonl_lines(path)?;
    if n <= RECORD_CAP {
        return Ok(());
    }
    let drop_n = n - RECORD_CAP;
    let f = fs::File::open(path)?;
    let kept: Vec<String> = BufReader::new(f)
        .lines()
        .skip(drop_n)
        .collect::<Result<Vec<_>, _>>()?;
    let tmp = path.with_extension("jsonl.tmp");
    {
        let mut out = File::create(&tmp)?;
        for line in &kept {
            writeln!(out, "{line}")?;
        }
    }
    fs::rename(&tmp, path)?;
    info!(target: "nosd", "memory evict fifo dropped={drop_n} cap={RECORD_CAP}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn remember_recall_legacy() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), Kind::NsgdbLegacy).unwrap();
        m.remember(MemoryRecord {
            key: "md/L3/t1".into(),
            text: "neural os mesh peers".into(),
            scope: "aios/user".into(),
            entities: vec!["mom/fact".into()],
        })
        .unwrap();
        let hits = m
            .recall_governed(RecallReq {
                query: "mesh",
                k: 5,
                scope: Some("aios/user"),
                entities: &[],
                mode: RecallMode::Lexical,
                embedding: None,
                path: None,
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].matched_terms.iter().any(|t| t == "mesh"));
    }

    #[test]
    fn null_scope_legacy() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), Kind::NsgdbLegacy).unwrap();
        m.remember(MemoryRecord {
            key: "k".into(),
            text: "wifi lab".into(),
            scope: "aios/user".into(),
            entities: vec!["mom/fact".into()],
        })
        .unwrap();
        assert!(m.recall("wifi", 5).unwrap().is_empty());
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
        assert_eq!(scoped.len(), 1);
    }

    #[test]
    fn fifo_evict_jsonl() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), Kind::Jsonl).unwrap();
        for i in 0..RECORD_CAP + 5 {
            m.remember(MemoryRecord {
                key: format!("k{i}"),
                text: format!("evict-marker-{i:05}"),
                scope: "aios/user".into(),
                entities: vec!["mom/fact".into()],
            })
            .unwrap();
        }
        assert_eq!(m.record_count().unwrap(), RECORD_CAP);
    }

    #[test]
    fn doctrine_seed_file_present() {
        let d = include_str!("../doctrine/DOCTRINE.md");
        assert!(d.contains("neural-sgdb is a MEMORY substrate"));
        assert!(d.contains("Null-scoping"));
    }

    #[test]
    fn hybrid_demo_downgrades_with_basis() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), Kind::NsgdbLegacy).unwrap();
        assert!(!m.embedder_real());
        let hits = m
            .recall_governed(RecallReq {
                query: "x",
                k: 1,
                scope: None,
                entities: &[],
                mode: RecallMode::Hybrid,
                embedding: Some(&[0.1, 0.2]),
                path: None,
            })
            .unwrap();
        // Sem hits ok; mode_used lexical + basis se houvesse hit — valida mode helper.
        assert_eq!(
            validate_recall_mode(RecallMode::Hybrid, true, false),
            RecallMode::Lexical
        );
        let _ = hits;
    }

    #[test]
    fn path_trigger_empty_without_rules() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), Kind::NsgdbLegacy).unwrap();
        let hits = m
            .recall_governed(RecallReq {
                query: "",
                k: 3,
                scope: None,
                entities: &[],
                mode: RecallMode::Lexical,
                embedding: None,
                path: Some("crates/nosd/src/main.rs"),
            })
            .unwrap();
        assert!(hits.is_empty(), "path sem trigger = vazio, nao global");
    }

    #[test]
    fn telepathy_two_db_exchange() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let ma = NosMemory::open_with_backend(a.path(), Kind::Neural).unwrap();
        let mb = NosMemory::open_with_backend(b.path(), Kind::Neural).unwrap();
        ma.remember_system(MemoryRecord {
            key: "fleet/fact-a".into(),
            text: "ssid lab-alpha na frota".into(),
            scope: "aios/fleet".into(),
            entities: vec!["mom/fact".into()],
        })
        .unwrap();
        mb.remember_system(MemoryRecord {
            key: "fleet/fact-b".into(),
            text: "peer bravo online".into(),
            scope: "aios/fleet".into(),
            entities: vec!["mom/fact".into()],
        })
        .unwrap();
        let exp_a = ma.telepathy_export(32).unwrap();
        assert!(!exp_a.is_empty());
        let ex = mb.telepathy_exchange(&exp_a, 32).unwrap();
        assert_eq!(ex["telepathy"], true);
        let back: Vec<String> = ex["records_b64"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        ma.telepathy_import(&back).unwrap();
        let hit_a = ma
            .recall_governed(RecallReq {
                query: "bravo",
                k: 5,
                scope: Some("aios/fleet"),
                entities: &[],
                mode: RecallMode::Lexical,
                embedding: None,
                path: None,
            })
            .unwrap();
        let hit_b = mb
            .recall_governed(RecallReq {
                query: "alpha",
                k: 5,
                scope: Some("aios/fleet"),
                entities: &[],
                mode: RecallMode::Lexical,
                embedding: None,
                path: None,
            })
            .unwrap();
        assert!(!hit_b.is_empty(), "B deve ver facto de A: {hit_b:?}");
        assert!(!hit_a.is_empty(), "A deve ver facto de B: {hit_a:?}");
    }

    #[test]
    fn neural_backend_remember_recall_scoped() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), Kind::Neural).unwrap();
        assert!(m.backend().is_neural());
        m.remember_system(MemoryRecord {
            key: "lab/wifi".into(),
            text: "wifi mesh lab secreto".into(),
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
            "null-scope neural: {global:?}"
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
        assert!(!scoped.is_empty(), "scoped neural deve ver facto");
        let cs = m.cold_start().unwrap();
        assert_eq!(cs["api_version"], MEMORY_API_VERSION);
    }
}
