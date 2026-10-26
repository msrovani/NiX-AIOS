//! Length-prefixed segment files (`seg.bin`) — NOT JSONL.
//!
//! Honesty: this is a local append log per layer, **not** full MCP neural-sgdb (no CRDT/BQ/ART).

use crate::{MemoryError, MemoryRecord, RECORD_CAP};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::info;

pub const SEG_MAGIC: [u8; 4] = *b"NSGS";
pub const SEG_VERSION: u32 = 1;
/// L4+ stub file header only — recall skips L4 until CRDT wiring.
pub const L4_STUB_MAGIC: [u8; 4] = *b"NSG4";
pub const L4_STUB_VERSION: u32 = 0;

fn layer_dir(root: &Path, layer: u8) -> PathBuf {
    root.join(format!("L{layer}")).join("seg.bin")
}

fn layer_from_key(key: &str) -> u8 {
    if key.starts_with("md/L2/") {
        2
    } else if key.starts_with("md/L3/") {
        3
    } else if key.starts_with("md/L4/") || key.starts_with("md/L5/") {
        4
    } else {
        3
    }
}

#[derive(Clone)]
pub struct NsgdbSegStore {
    root: PathBuf,
}

impl NsgdbSegStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, MemoryError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("L2"))?;
        fs::create_dir_all(root.join("L3"))?;
        fs::create_dir_all(root.join("L4"))?;
        for layer in [2u8, 3] {
            ensure_seg_file(&layer_dir(&root, layer))?;
        }
        ensure_l4_stub(&layer_dir(&root, 4))?;
        Ok(Self { root })
    }

    pub fn remember(&self, rec: MemoryRecord) -> Result<(), MemoryError> {
        let layer = layer_from_key(&rec.key);
        if layer == 4 {
            // Stub: header-only file; L4+ CRDT AWAITING — recall ignores L4.
            let _ = rec;
            return Ok(());
        }
        let path = layer_dir(&self.root, layer);
        append_record(&path, &rec)?;
        evict_fifo(&path, RECORD_CAP)?;
        Ok(())
    }

    pub fn recall_layers(&self, layers: &[u8]) -> Result<Vec<MemoryRecord>, MemoryError> {
        let mut out = Vec::new();
        for &layer in layers {
            if layer == 4 {
                continue;
            }
            let path = layer_dir(&self.root, layer);
            out.extend(read_all_records(&path)?);
        }
        Ok(out)
    }

    pub fn record_count(&self, layers: &[u8]) -> Result<usize, MemoryError> {
        let mut n = 0usize;
        for &layer in layers {
            if layer == 4 {
                continue;
            }
            n += read_all_records(&layer_dir(&self.root, layer))?.len();
        }
        Ok(n)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn ensure_seg_file(path: &Path) -> Result<(), MemoryError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = File::create(path)?;
    f.write_all(&SEG_MAGIC)?;
    f.write_all(&SEG_VERSION.to_le_bytes())?;
    Ok(())
}

fn ensure_l4_stub(path: &Path) -> Result<(), MemoryError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = File::create(path)?;
    f.write_all(&L4_STUB_MAGIC)?;
    f.write_all(&L4_STUB_VERSION.to_le_bytes())?;
    Ok(())
}

fn encode_payload(rec: &MemoryRecord) -> Result<Vec<u8>, MemoryError> {
    serde_json::to_vec(rec).map_err(MemoryError::from)
}

fn upgrade_l4_stub_to_seg(path: &Path) -> Result<(), MemoryError> {
    if !path.exists() {
        return Ok(());
    }
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() {
        return Ok(());
    }
    if magic == L4_STUB_MAGIC {
        drop(f);
        fs::remove_file(path)?;
    }
    Ok(())
}

fn append_record(path: &Path, rec: &MemoryRecord) -> Result<(), MemoryError> {
    upgrade_l4_stub_to_seg(path)?;
    ensure_seg_file(path)?;
    let payload = encode_payload(rec)?;
    let len = u32::try_from(payload.len()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "record too large")
    })?;
    let mut f = OpenOptions::new().append(true).open(path)?;
    f.write_all(&len.to_le_bytes())?;
    f.write_all(&payload)?;
    Ok(())
}

fn read_all_records(path: &Path) -> Result<Vec<MemoryRecord>, MemoryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic)?;
    if magic == L4_STUB_MAGIC {
        let _ = read_u32(&mut f)?;
        return Ok(Vec::new());
    }
    if magic != SEG_MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad seg magic",
        )
        .into());
    }
    let _ver = read_u32(&mut f)?;
    let mut records = Vec::new();
    loop {
        let len = match read_u32(&mut f) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if len == 0 {
            continue;
        }
        let mut buf = vec![0u8; len as usize];
        f.read_exact(&mut buf)?;
        let rec: MemoryRecord = serde_json::from_slice(&buf)?;
        records.push(rec);
    }
    Ok(records)
}

fn read_u32(r: &mut File) -> Result<u32, std::io::Error> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn evict_fifo(path: &Path, cap: usize) -> Result<(), MemoryError> {
    let records = read_all_records(path)?;
    if records.len() <= cap {
        return Ok(());
    }
    let drop_n = records.len() - cap;
    let kept = records.into_iter().skip(drop_n).collect::<Vec<_>>();
    rewrite_seg(path, &kept)?;
    info!(
        target: "nosd",
        "nsgdb seg evict fifo path={} dropped={drop_n} cap={cap}",
        path.display()
    );
    Ok(())
}

fn rewrite_seg(path: &Path, kept: &[MemoryRecord]) -> Result<(), MemoryError> {
    let tmp = path.with_extension("bin.tmp");
    {
        let mut f = File::create(&tmp)?;
        f.write_all(&SEG_MAGIC)?;
        f.write_all(&SEG_VERSION.to_le_bytes())?;
        for rec in kept {
            let payload = encode_payload(rec)?;
            let len = u32::try_from(payload.len()).unwrap();
            f.write_all(&len.to_le_bytes())?;
            f.write_all(&payload)?;
        }
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn length_prefixed_not_jsonl() {
        let d = tempdir().unwrap();
        let root = d.path().join("nsgdb");
        let store = NsgdbSegStore::open(&root).unwrap();
        store
            .remember(MemoryRecord {
                key: "md/L3/t".into(),
                text: "alpha beta".into(),
                scope: "s".into(),
                entities: vec![],
            })
            .unwrap();
        let raw = fs::read(layer_dir(&root, 3)).unwrap();
        assert!(raw.starts_with(&SEG_MAGIC));
        assert!(!raw.windows(2).any(|w| w == b"\n{"), "not line-delimited JSONL");
    }

    #[test]
    fn l2_l3_recall_l4_stub() {
        let d = tempdir().unwrap();
        let root = d.path().join("nsgdb");
        let store = NsgdbSegStore::open(&root).unwrap();
        store
            .remember(MemoryRecord {
                key: "md/L2/ep".into(),
                text: "mesh episodic".into(),
                scope: "test".into(),
                entities: vec![],
            })
            .unwrap();
        store
            .remember(MemoryRecord {
                key: "md/L4/stub".into(),
                text: "should not recall".into(),
                scope: "test".into(),
                entities: vec![],
            })
            .unwrap();
        let recs = store.recall_layers(&[2, 3]).unwrap();
        assert_eq!(recs.len(), 1);
        let l4 = fs::read(layer_dir(&root, 4)).unwrap();
        assert!(l4.starts_with(&L4_STUB_MAGIC), "L4 remains stub header");
    }
}
