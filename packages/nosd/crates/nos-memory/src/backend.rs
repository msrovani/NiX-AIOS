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
    pub fn from_env() -> Self {
        match std::env::var("NOSD_MEMORY_BACKEND")
            .unwrap_or_else(|_| "nsgdb".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "jsonl" => Self::Jsonl,
            _ => Self::Nsgdb,
        }
    }

    pub fn storage_path(data_dir: &Path) -> PathBuf {
        match Self::from_env() {
            Self::Jsonl => data_dir.join("memories.jsonl"),
            Self::Nsgdb => data_dir.join("nsgdb"),
        }
    }

    pub fn ensure_dirs(data_dir: &Path) -> std::io::Result<()> {
        match Self::from_env() {
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

    /// Camadas activas até wiring neural-sgdb MCP real (BQ/semantic = AWAITING).
    pub fn layers_supported(self) -> &'static [MemoryLayer] {
        match self {
            Self::Jsonl => &[MemoryLayer::L3Pattern],
            Self::Nsgdb => &[MemoryLayer::L2Episodic, MemoryLayer::L3Pattern, MemoryLayer::L4Stub],
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
        std::env::remove_var("NOSD_MEMORY_BACKEND");
        let p = MemoryBackendKind::storage_path(d.path());
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
    fn jsonl_env_path() {
        let d = tempdir().unwrap();
        std::env::set_var("NOSD_MEMORY_BACKEND", "jsonl");
        let p = MemoryBackendKind::storage_path(d.path());
        assert!(p.ends_with("memories.jsonl"));
        std::env::remove_var("NOSD_MEMORY_BACKEND");
    }
}
