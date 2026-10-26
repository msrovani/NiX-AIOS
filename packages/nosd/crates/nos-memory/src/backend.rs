//! Backends — honesty DB.1:
//!
//! - `NOSD_MEMORY_BACKEND=jsonl` → append-only `data_dir/memories.jsonl` (opt-in)
//! - default `nsgdb` → length-prefixed `data_dir/nsgdb/L{2,3,4}/seg.bin` (NOT JSONL).
//!   Não é motor MCP neural-sgdb completo (sem CRDT/BQ/semantic host).

use std::path::{Path, PathBuf};

/// Camadas NSGDB efectivamente suportadas neste crate (honest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLayer {
    L2Episodic,
    L3Pattern,
    /// Header stub em `L4/seg.bin`; recall activo = L2+L3 apenas.
    L4Stub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBackendKind {
    Jsonl,
    Nsgdb,
}

impl MemoryBackendKind {
    /// Parser puro (testável sem `std::env`).
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "jsonl" => Self::Jsonl,
            _ => Self::Nsgdb,
        }
    }

    pub fn from_env() -> Self {
        Self::parse(&std::env::var("NOSD_MEMORY_BACKEND").unwrap_or_else(|_| "nsgdb".into()))
    }

    pub fn storage_path_for(self, data_dir: &Path) -> PathBuf {
        match self {
            Self::Jsonl => data_dir.join("memories.jsonl"),
            Self::Nsgdb => data_dir.join("nsgdb"),
        }
    }

    pub fn storage_path(data_dir: &Path) -> PathBuf {
        Self::from_env().storage_path_for(data_dir)
    }

    pub fn ensure_dirs_for(self, data_dir: &Path) -> std::io::Result<()> {
        match self {
            Self::Jsonl => {
                let path = data_dir.join("memories.jsonl");
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                Ok(())
            }
            Self::Nsgdb => {
                let root = data_dir.join("nsgdb");
                std::fs::create_dir_all(root.join("L2"))?;
                std::fs::create_dir_all(root.join("L3"))?;
                std::fs::create_dir_all(root.join("L4"))?;
                Ok(())
            }
        }
    }

    pub fn ensure_dirs(data_dir: &Path) -> std::io::Result<()> {
        Self::from_env().ensure_dirs_for(data_dir)
    }

    /// Camadas activas até wiring neural-sgdb MCP real (BQ/semantic = AWAITING).
    pub fn layers_supported(self) -> &'static [MemoryLayer] {
        match self {
            Self::Jsonl => &[MemoryLayer::L3Pattern],
            Self::Nsgdb => &[
                MemoryLayer::L2Episodic,
                MemoryLayer::L3Pattern,
                MemoryLayer::L4Stub,
            ],
        }
    }

    pub fn uses_jsonl_on_disk(self) -> bool {
        matches!(self, Self::Jsonl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn nsgdb_default_dir() {
        let d = tempdir().unwrap();
        let p = MemoryBackendKind::Nsgdb.storage_path_for(d.path());
        assert!(p.ends_with("nsgdb"));
    }

    #[test]
    fn nsgdb_binary_layers_l2_l3() {
        let kind = MemoryBackendKind::Nsgdb;
        assert!(!kind.uses_jsonl_on_disk());
        assert!(kind.layers_supported().contains(&MemoryLayer::L2Episodic));
        assert!(kind.layers_supported().contains(&MemoryLayer::L3Pattern));
    }

    #[test]
    fn jsonl_path_is_file() {
        let d = tempdir().unwrap();
        let p = MemoryBackendKind::parse("jsonl").storage_path_for(d.path());
        assert!(p.ends_with("memories.jsonl"));
        assert_eq!(MemoryBackendKind::parse("JSONL"), MemoryBackendKind::Jsonl);
        assert_eq!(MemoryBackendKind::parse("nsgdb"), MemoryBackendKind::Nsgdb);
    }
}
