//! nos-memory — substrato alinhado a neural-sgdb (ADR-0091) **honesty DB.1**.
//!
//! `NOSD_MEMORY_BACKEND=nsgdb` (default) grava registos **length-prefixed** em
//! `data_dir/nsgdb/L{2,3,4}/seg.bin` — não JSONL; não é motor NSGDB MCP completo.
//! Backend `jsonl` opt-in usa `data_dir/memories.jsonl`. Recall lexical (substring);
//! cap FIFO 1024 por camada (nsgdb) ou ficheiro (jsonl).

mod backend;
mod seg;

use backend::MemoryBackendKind;
use parking_lot::Mutex;
use seg::NsgdbSegStore;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

pub use backend::{MemoryBackendKind as MemoryBackend, MemoryLayer};

pub const RECORD_CAP: usize = 1024;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
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
    pub text: String,
    pub score: f32,
    pub matched_terms: Vec<String>,
}

#[derive(Clone)]
enum Storage {
    Jsonl(PathBuf),
    Nsgdb(NsgdbSegStore),
}

#[derive(Clone)]
pub struct NosMemory {
    storage: Storage,
    backend: MemoryBackendKind,
    lock: Arc<Mutex<()>>,
}

impl NosMemory {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, MemoryError> {
        Self::open_with_backend(dir, MemoryBackendKind::from_env())
    }

    pub fn open_with_backend(dir: impl AsRef<Path>, backend: MemoryBackendKind) -> Result<Self, MemoryError> {
        let dir = dir.as_ref();
        MemoryBackendKind::ensure_dirs(dir)?;
        let storage = match backend {
            MemoryBackendKind::Jsonl => {
                let path = dir.join("memories.jsonl");
                if !path.exists() {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    File::create(&path)?;
                }
                Storage::Jsonl(path)
            }
            MemoryBackendKind::Nsgdb => {
                let root = dir.join("nsgdb");
                Storage::Nsgdb(NsgdbSegStore::open(&root)?)
            }
        };
        info!(
            target: "nosd",
            "memory open backend={:?} jsonl_on_disk={}",
            backend,
            backend.uses_jsonl_on_disk()
        );
        Ok(Self {
            storage,
            backend,
            lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn backend(&self) -> MemoryBackendKind {
        self.backend
    }

    pub fn layers_supported(&self) -> &'static [MemoryLayer] {
        self.backend.layers_supported()
    }

    pub fn remember(&self, rec: MemoryRecord) -> Result<String, MemoryError> {
        let _g = self.lock.lock();
        match &self.storage {
            Storage::Jsonl(path) => {
                let mut f = OpenOptions::new().append(true).open(path)?;
                let line = serde_json::to_string(&rec)?;
                writeln!(f, "{line}")?;
                evict_jsonl_fifo(path)?;
            }
            Storage::Nsgdb(store) => {
                store.remember(rec.clone())?;
            }
        }
        Ok(rec.key.clone())
    }

    pub fn recall(&self, query: &str, k: usize) -> Result<Vec<Hit>, MemoryError> {
        self.recall_lexical(query, k)
    }

    pub fn recall_lexical(&self, query: &str, k: usize) -> Result<Vec<Hit>, MemoryError> {
        let _g = self.lock.lock();
        let records = match &self.storage {
            Storage::Jsonl(path) => read_jsonl_records(path)?,
            Storage::Nsgdb(store) => store.recall_layers(&[2, 3])?,
        };
        Ok(score_hits(query, k, records))
    }

    pub fn health(&self) -> bool {
        match &self.storage {
            Storage::Jsonl(path) => path.exists(),
            Storage::Nsgdb(store) => store.root().exists(),
        }
    }

    pub fn record_count(&self) -> Result<usize, MemoryError> {
        let _g = self.lock.lock();
        match &self.storage {
            Storage::Jsonl(path) => Ok(count_jsonl_lines(path)?),
            Storage::Nsgdb(store) => store.record_count(&[2, 3]),
        }
    }
}

fn score_hits(query: &str, k: usize, records: Vec<MemoryRecord>) -> Vec<Hit> {
    let terms: Vec<&str> = query.split_whitespace().filter(|t| t.len() > 1).collect();
    let mut hits = Vec::new();
    for rec in records {
        let lower = rec.text.to_ascii_lowercase();
        let mut matched = Vec::new();
        let mut score = 0.0f32;
        for t in &terms {
            let tl = t.to_ascii_lowercase();
            if lower.contains(&tl) {
                matched.push((*t).to_string());
                score += 1.0;
            }
        }
        if score > 0.0 || terms.is_empty() {
            hits.push(Hit {
                key: rec.key,
                text: rec.text,
                score,
                matched_terms: matched,
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
    fn remember_recall_nsgdb_seg() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::Nsgdb).unwrap();
        m.remember(MemoryRecord {
            key: "md/L3/t1".into(),
            text: "neural os mesh peers".into(),
            scope: "project/neural-os-core".into(),
            entities: vec!["mom/fact".into()],
        })
        .unwrap();
        let hits = m.recall("mesh", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].matched_terms.iter().any(|t| t == "mesh"));
        assert_eq!(m.backend(), MemoryBackendKind::Nsgdb);
        assert!(!m.backend().uses_jsonl_on_disk());
    }

    #[test]
    fn fifo_evict_jsonl() {
        let d = tempdir().unwrap();
        let m = NosMemory::open_with_backend(d.path(), MemoryBackendKind::Jsonl).unwrap();
        for i in 0..RECORD_CAP + 5 {
            m.remember(MemoryRecord {
                key: format!("k{i}"),
                text: format!("evict-marker-{i:05}"),
                scope: "test".into(),
                entities: vec![],
            })
            .unwrap();
        }
        assert_eq!(m.record_count().unwrap(), RECORD_CAP);
        let hits = m.recall("evict-marker-00000", 5).unwrap();
        assert!(hits.is_empty(), "oldest records evicted");
        assert!(!m.recall("evict-marker-01027", 5).unwrap().is_empty());
    }
}
