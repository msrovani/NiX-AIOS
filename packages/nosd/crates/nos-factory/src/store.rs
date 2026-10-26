//! Federated PackageStore — imported mesh/public entries, sandbox + hash gate, FIFO cap.
//! Índices federados (P.12): privados allowlist + públicos GH/GL/crates via Seek.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;

use crate::seek::{CandidateSource, SkillCandidate};

pub const PACKAGE_STORE_CAP: usize = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportProvenance {
    ImportedMesh,
    ImportedPublic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SandboxStatus {
    Pending,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportMeta {
    pub name: String,
    /// ADR-0052 hex16 FNV-1a64 do corpo; absent = unsigned deny.
    pub content_hash: Option<String>,
    /// Ed25519 hex128 (64 bytes) sobre o body; se presente, verify obrigatório (P.12 residual).
    pub signature: Option<String>,
    /// Chave pública trusted hex64 (32 bytes); exigida se `signature` presente.
    pub public_key: Option<String>,
    pub provenance: ImportProvenance,
    pub sandbox_status: SandboxStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredPackage {
    pub meta: ImportMeta,
    pub body_len: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    #[error("unsigned: content_hash required")]
    Unsigned,
    #[error("empty name")]
    EmptyName,
    #[error("sandbox not passed")]
    SandboxNotPassed,
    #[error("content_hash mismatch")]
    HashMismatch,
    #[error("invalid content_hash hex")]
    InvalidHash,
    #[error("invalid ed25519 signature/key hex")]
    InvalidSignatureHex,
    #[error("ed25519 verify failed")]
    SignatureBad,
    #[error("signature without public_key")]
    MissingPublicKey,
}

#[derive(Debug, Default)]
pub struct PackageStore {
    entries: VecDeque<StoredPackage>,
}

impl PackageStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &StoredPackage> {
        self.entries.iter()
    }

    /// Install imported artifact bytes; exige `sandbox_status=Passed` + FNV hex16 do body.
    pub fn install_imported(
        &mut self,
        meta: ImportMeta,
        body: &[u8],
    ) -> Result<&StoredPackage, StoreError> {
        if meta.name.trim().is_empty() {
            return Err(StoreError::EmptyName);
        }
        if meta.sandbox_status != SandboxStatus::Passed {
            return Err(StoreError::SandboxNotPassed);
        }
        let hash = meta.content_hash.as_ref().map(|s| s.trim()).unwrap_or("");
        if hash.is_empty() {
            return Err(StoreError::Unsigned);
        }
        let expected = parse_hash_hex16(hash).ok_or(StoreError::InvalidHash)?;
        let actual = fnv1a64(body);
        if expected != actual {
            return Err(StoreError::HashMismatch);
        }
        if meta
            .signature
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
        {
            let pk = meta
                .public_key
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or(StoreError::MissingPublicKey)?;
            let sig = meta.signature.as_ref().map(|s| s.trim()).unwrap_or("");
            verify_ed25519(body, pk, sig)?;
        }

        if self.entries.len() >= PACKAGE_STORE_CAP {
            self.entries.pop_front();
        }

        let pkg = StoredPackage {
            meta,
            body_len: body.len(),
        };
        self.entries.push_back(pkg);
        Ok(self.entries.back().expect("just pushed"))
    }
}

pub const FEDERATED_INDEX_CAP: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FederatedIndexEntry {
    pub name: String,
    pub url: String,
    pub source: CandidateSource,
    pub score: f32,
    pub license: Option<String>,
    pub escalate_license: bool,
    pub sandbox_status: SandboxStatus,
}

/// Catálogo federado (índices) — distinto do install de bytes; FIFO.
#[derive(Debug, Default)]
pub struct FederatedIndex {
    entries: VecDeque<FederatedIndexEntry>,
}

impl FederatedIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = &FederatedIndexEntry> {
        self.entries.iter()
    }

    /// Ingere candidatos do Seek (privados + públicos). GPL → sandbox Pending.
    pub fn ingest_seek_candidates(&mut self, cands: &[SkillCandidate]) -> usize {
        let mut n = 0usize;
        for c in cands {
            let sandbox_status = if c.escalate_license {
                SandboxStatus::Pending
            } else {
                SandboxStatus::Passed
            };
            let e = FederatedIndexEntry {
                name: c.name.clone(),
                url: c.url.clone(),
                source: c.source,
                score: c.score,
                license: c.license.clone(),
                escalate_license: c.escalate_license,
                sandbox_status,
            };
            // replace same url
            self.entries.retain(|x| x.url != e.url);
            if self.entries.len() >= FEDERATED_INDEX_CAP {
                self.entries.pop_front();
            }
            self.entries.push_back(e);
            n += 1;
        }
        n
    }

    pub fn best_for_goal(&self, goal: &str) -> Option<&FederatedIndexEntry> {
        let g = goal.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.name
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|t| t.len() >= 3 && g.contains(t))
                    || g.split_whitespace()
                        .any(|t| e.name.to_lowercase().contains(t))
            })
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

pub use nos_core::{fnv1a64, fnv1a64_hex16};

fn parse_hash_hex16(s: &str) -> Option<u64> {
    let s = s.trim().trim_start_matches("0x");
    if s.len() > 16 {
        return None;
    }
    u64::from_str_radix(s, 16).ok()
}

fn parse_hex_bytes(s: &str, expect: usize) -> Option<Vec<u8>> {
    let s = s.trim().trim_start_matches("0x");
    if s.len() != expect * 2 {
        return None;
    }
    let mut out = Vec::with_capacity(expect);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = from_hex(bytes[i])?;
        let lo = from_hex(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    (out.len() == expect).then_some(out)
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn verify_ed25519(body: &[u8], pk_hex: &str, sig_hex: &str) -> Result<(), StoreError> {
    let pk_bytes = parse_hex_bytes(pk_hex, 32).ok_or(StoreError::InvalidSignatureHex)?;
    let sig_bytes = parse_hex_bytes(sig_hex, 64).ok_or(StoreError::InvalidSignatureHex)?;
    let pk = ed25519_compact::PublicKey::from_slice(&pk_bytes)
        .map_err(|_| StoreError::InvalidSignatureHex)?;
    let sig = ed25519_compact::Signature::from_slice(&sig_bytes)
        .map_err(|_| StoreError::InvalidSignatureHex)?;
    pk.verify(body, &sig)
        .map_err(|_| StoreError::SignatureBad)?;
    Ok(())
}

/// Hex lowercase de bytes (testes / tools).
pub fn bytes_to_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(data.len() * 2);
    for b in data {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_without_hash() {
        let mut store = PackageStore::new();
        let meta = ImportMeta {
            name: "x".into(),
            content_hash: None,
            signature: None,
            public_key: None,
            provenance: ImportProvenance::ImportedPublic,
            sandbox_status: SandboxStatus::Passed,
        };
        assert_eq!(
            store.install_imported(meta, b"body"),
            Err(StoreError::Unsigned)
        );
    }

    #[test]
    fn deny_sandbox_pending() {
        let mut store = PackageStore::new();
        let body = b"signed-body";
        let meta = ImportMeta {
            name: "x".into(),
            content_hash: Some(fnv1a64_hex16(body)),
            signature: None,
            public_key: None,
            provenance: ImportProvenance::ImportedPublic,
            sandbox_status: SandboxStatus::Pending,
        };
        assert_eq!(
            store.install_imported(meta, body),
            Err(StoreError::SandboxNotPassed)
        );
    }

    #[test]
    fn install_with_hash_ok() {
        let mut store = PackageStore::new();
        let body = b"---";
        let meta = ImportMeta {
            name: "echo".into(),
            content_hash: Some(fnv1a64_hex16(body)),
            signature: None,
            public_key: None,
            provenance: ImportProvenance::ImportedMesh,
            sandbox_status: SandboxStatus::Passed,
        };
        let p = store.install_imported(meta, body).unwrap();
        assert_eq!(p.meta.name, "echo");
        assert_eq!(p.meta.provenance, ImportProvenance::ImportedMesh);
    }

    #[test]
    fn hash_mismatch() {
        let mut store = PackageStore::new();
        let meta = ImportMeta {
            name: "bad".into(),
            content_hash: Some("deadbeef01234567".into()),
            signature: None,
            public_key: None,
            provenance: ImportProvenance::ImportedPublic,
            sandbox_status: SandboxStatus::Passed,
        };
        assert_eq!(
            store.install_imported(meta, b"other"),
            Err(StoreError::HashMismatch)
        );
    }

    #[test]
    fn fifo_evicts_oldest() {
        let mut store = PackageStore::new();
        for i in 0..PACKAGE_STORE_CAP + 3 {
            let body = format!("body-{i}").into_bytes();
            let meta = ImportMeta {
                name: format!("pkg-{i}"),
                content_hash: Some(fnv1a64_hex16(&body)),
                signature: None,
                public_key: None,
                provenance: ImportProvenance::ImportedPublic,
                sandbox_status: SandboxStatus::Passed,
            };
            store.install_imported(meta, &body).unwrap();
        }
        assert_eq!(store.len(), PACKAGE_STORE_CAP);
        assert_eq!(store.entries().next().unwrap().meta.name, "pkg-3");
    }

    #[test]
    fn ed25519_verify_ok() {
        let mut store = PackageStore::new();
        let body = b"signed-artifact-body";
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([7u8; 32]));
        let sig = kp.sk.sign(body, None);
        let meta = ImportMeta {
            name: "signed".into(),
            content_hash: Some(fnv1a64_hex16(body)),
            signature: Some(bytes_to_hex(sig.as_ref())),
            public_key: Some(bytes_to_hex(kp.pk.as_ref())),
            provenance: ImportProvenance::ImportedPublic,
            sandbox_status: SandboxStatus::Passed,
        };
        assert!(store.install_imported(meta, body).is_ok());
    }

    #[test]
    fn federated_ingest_and_best() {
        let mut idx = FederatedIndex::new();
        let cands = vec![SkillCandidate {
            source: CandidateSource::GitHub,
            name: "mesh-skill".into(),
            url: "https://github.com/x/y".into(),
            score: 0.8,
            license: Some("MIT".into()),
            escalate_license: false,
        }];
        assert_eq!(idx.ingest_seek_candidates(&cands), 1);
        assert!(idx.best_for_goal("mesh skill").is_some());
    }

    #[test]
    fn ed25519_bad_sig_denied() {
        let mut store = PackageStore::new();
        let body = b"body";
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([9u8; 32]));
        let meta = ImportMeta {
            name: "bad-sig".into(),
            content_hash: Some(fnv1a64_hex16(body)),
            signature: Some("00".repeat(64)),
            public_key: Some(bytes_to_hex(kp.pk.as_ref())),
            provenance: ImportProvenance::ImportedPublic,
            sandbox_status: SandboxStatus::Passed,
        };
        assert_eq!(
            store.install_imported(meta, body),
            Err(StoreError::SignatureBad)
        );
    }
}
