//! Backends de memória cognitiva.
//!
//! - `neural` (default) → FileStorage real em `data_dir/memory/nsgdb/`
//! - `nsgdb_legacy` → seg.bin length-prefixed em `data_dir/nsgdb/L{2,3}/`
//! - `jsonl` → `data_dir/memories.jsonl` (lab)

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLayer {
    L2Episodic,
    L3Pattern,
    L4Stub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBackendKind {
    Neural,
    NsgdbLegacy,
    Jsonl,
}

impl MemoryBackendKind {
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "jsonl" => Self::Jsonl,
            "nsgdb_legacy" | "legacy" | "nsgdb" => Self::NsgdbLegacy,
            _ => Self::Neural,
        }
    }

    pub fn from_env() -> Self {
        Self::parse(&std::env::var("NOSD_MEMORY_BACKEND").unwrap_or_else(|_| "neural".into()))
    }

    pub fn storage_path_for(self, data_dir: &Path) -> PathBuf {
        match self {
            Self::Jsonl => data_dir.join("memories.jsonl"),
            Self::NsgdbLegacy => data_dir.join("nsgdb"),
            Self::Neural => data_dir.join("memory").join("nsgdb"),
        }
    }

    pub fn ensure_dirs_for(self, data_dir: &Path) -> std::io::Result<()> {
        match self {
            Self::Jsonl => {
                if let Some(parent) = data_dir.join("memories.jsonl").parent() {
                    std::fs::create_dir_all(parent)?;
                }
                Ok(())
            }
            Self::NsgdbLegacy => {
                let root = data_dir.join("nsgdb");
                std::fs::create_dir_all(root.join("L2"))?;
                std::fs::create_dir_all(root.join("L3"))?;
                std::fs::create_dir_all(root.join("L4"))?;
                Ok(())
            }
            Self::Neural => {
                std::fs::create_dir_all(data_dir.join("memory").join("nsgdb"))?;
                Ok(())
            }
        }
    }

    pub fn ensure_dirs(data_dir: &Path) -> std::io::Result<()> {
        Self::from_env().ensure_dirs_for(data_dir)
    }

    pub fn layers_supported(self) -> &'static [MemoryLayer] {
        match self {
            Self::Jsonl => &[MemoryLayer::L3Pattern],
            Self::NsgdbLegacy | Self::Neural => &[
                MemoryLayer::L2Episodic,
                MemoryLayer::L3Pattern,
                MemoryLayer::L4Stub,
            ],
        }
    }

    pub fn uses_jsonl_on_disk(self) -> bool {
        matches!(self, Self::Jsonl)
    }

    pub fn is_neural(self) -> bool {
        matches!(self, Self::Neural)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn neural_default_path() {
        let d = tempdir().unwrap();
        let p = MemoryBackendKind::Neural.storage_path_for(d.path());
        assert!(p.ends_with("memory/nsgdb") || p.ends_with(r"memory\nsgdb"));
    }

    #[test]
    fn parse_aliases() {
        assert_eq!(
            MemoryBackendKind::parse("neural"),
            MemoryBackendKind::Neural
        );
        assert_eq!(
            MemoryBackendKind::parse("nsgdb_legacy"),
            MemoryBackendKind::NsgdbLegacy
        );
        assert_eq!(MemoryBackendKind::parse("jsonl"), MemoryBackendKind::Jsonl);
    }
}
