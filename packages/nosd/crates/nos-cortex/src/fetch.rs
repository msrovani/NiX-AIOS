//! Fetch GGUF (HTTPS / HuggingFace resolve) + sugestão de quant por VRAM.
//! Honesty: não é `hf-hub` crate completo; URL resolve + download streaming.

use crate::capacity::NodeCapacity;
use crate::offload::plan_n_gpu_layers;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("http: {0}")]
    Http(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad args: {0}")]
    BadArgs(&'static str),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantSuggest {
    /// Ex.: Q4_K_M
    pub quant: String,
    pub reason: String,
    pub suggest_ngl: u32,
    pub vram_free_mb: u64,
    pub engine_path: String,
}

/// Sugere quant GGUF + ngl a partir do probe (heurística, não HF ranking).
pub fn suggest_for_capacity(cap: &NodeCapacity) -> QuantSuggest {
    let vram = cap.vram_free_mb;
    let (quant, reason) = if vram >= 20_000 {
        ("Q5_K_M", "vram>=20GB → Q5_K_M")
    } else if vram >= 10_000 {
        ("Q4_K_M", "vram>=10GB → Q4_K_M")
    } else if vram >= 6_000 {
        ("Q4_K_S", "vram>=6GB → Q4_K_S")
    } else if vram >= 4_000 {
        ("Q3_K_M", "vram>=4GB → Q3_K_M")
    } else if vram > 0 {
        ("Q2_K", "vram baixa → Q2_K / CPU offload")
    } else {
        ("Q4_K_M", "vram_unknown → Q4_K_M + ngl=0 (CPU)")
    };
    let ngl = plan_n_gpu_layers(32, 180, vram, 256, 128, 256);
    let engine = if vram >= 6_000 && cap.vendor_hint == "nvidia" {
        "llama-gguf (CUDA ngl when NOSD_LLAMA_NGL/set)".into()
    } else {
        "llama-gguf (CPU/hybrid)".into()
    };
    QuantSuggest {
        quant: quant.into(),
        reason: reason.into(),
        suggest_ngl: ngl,
        vram_free_mb: vram,
        engine_path: engine,
    }
}

/// `https://huggingface.co/{repo}/resolve/main/{file}`
pub fn hf_resolve_url(repo: &str, file: &str) -> String {
    format!(
        "https://huggingface.co/{}/resolve/main/{}",
        repo.trim_matches('/'),
        file.trim_start_matches('/')
    )
}

/// Cap S-07: 8 GiB por download (modelo/firmware). Evita encher o disco.
pub const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// S-07: apenas basename; sem path traversal, absoluto, `.`/`..`.
fn safe_filename(filename: &str) -> Result<&str, FetchError> {
    let name = filename.trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err(FetchError::BadArgs("invalid filename"));
    }
    if name.contains('/') || name.contains('\\') || Path::new(name).is_absolute() {
        return Err(FetchError::BadArgs("filename must be a basename"));
    }
    Ok(name)
}

/// S-07: `.gguf` só é aceite com magic `GGUF`.
fn verify_gguf_magic(path: &Path) -> Result<(), FetchError> {
    let is_gguf = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !is_gguf {
        return Ok(());
    }
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic)?;
    if &magic != b"GGUF" {
        return Err(FetchError::BadArgs("download is not a GGUF file"));
    }
    Ok(())
}

/// Download para `dest_dir/filename` (cria dir). CapGate NET/FS = caller.
pub fn fetch_to_dir(url: &str, dest_dir: &Path, filename: &str) -> Result<PathBuf, FetchError> {
    if url.is_empty() {
        return Err(FetchError::BadArgs("url empty"));
    }
    let filename = safe_filename(filename)?;
    fs::create_dir_all(dest_dir)?;
    let dest = dest_dir.join(filename);
    if dest.exists() && dest.metadata().map(|m| m.len() > 1024).unwrap_or(false) {
        info!(target: "nosd", "fetch cache hit {}", dest.display());
        return Ok(dest);
    }
    info!(target: "nosd", "fetch GET {} → {}", url, dest.display());
    let resp = ureq::get(url)
        .set("User-Agent", "nosd-cortex/0.1 (NixAIOS)")
        .call()
        .map_err(|e| FetchError::Http(e.to_string()))?;
    if !(200..300).contains(&resp.status()) {
        return Err(FetchError::Http(format!("status {}", resp.status())));
    }
    if let Some(len) = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
    {
        if len > MAX_DOWNLOAD_BYTES {
            return Err(FetchError::Http(format!(
                "content-length {len} exceeds cap {MAX_DOWNLOAD_BYTES}"
            )));
        }
    }
    let tmp = dest.with_extension("partial");
    let mut file = File::create(&tmp)?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        total = total.saturating_add(n as u64);
        if total > MAX_DOWNLOAD_BYTES {
            drop(file);
            let _ = fs::remove_file(&tmp);
            return Err(FetchError::Http(format!(
                "download exceeds cap {MAX_DOWNLOAD_BYTES} bytes"
            )));
        }
    }
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, &dest)?;
    if let Err(e) = verify_gguf_magic(&dest) {
        let _ = fs::remove_file(&dest);
        return Err(e);
    }
    info!(target: "nosd", "fetch done bytes={} path={}", total, dest.display());
    Ok(dest)
}

pub fn fetch_hf_gguf(repo: &str, file: &str, dest_dir: &Path) -> Result<PathBuf, FetchError> {
    let url = hf_resolve_url(repo, file);
    let name = Path::new(file)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file);
    fetch_to_dir(&url, dest_dir, name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfModelHit {
    pub repo: String,
    pub downloads: u64,
    pub likes: u64,
    pub pipeline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPlan {
    pub capacity: QuantSuggest,
    pub search_query: String,
    pub candidates: Vec<HfModelHit>,
    /// Melhor palpite: repo + ficheiro GGUF com a quant sugerida.
    pub recommended_repo: String,
    pub recommended_file: String,
    pub note: String,
}

/// HF Hub API search (filter gguf). Sem token = público.
pub fn search_hf_gguf(query: &str, limit: u32) -> Result<Vec<HfModelHit>, FetchError> {
    let q = if query.trim().is_empty() {
        "gguf"
    } else {
        query.trim()
    };
    let lim = limit.clamp(1, 20);
    let url = format!(
        "https://huggingface.co/api/models?search={}&filter=gguf&sort=downloads&limit={}",
        urlencoding_minimal(q),
        lim
    );
    info!(target: "nosd", "hf search {}", url);
    let resp = ureq::get(&url)
        .set("User-Agent", "nosd-cortex/0.1 (NixAIOS)")
        .call()
        .map_err(|e| FetchError::Http(e.to_string()))?;
    if !(200..300).contains(&resp.status()) {
        return Err(FetchError::Http(format!("status {}", resp.status())));
    }
    let mut body = String::new();
    resp.into_reader()
        .read_to_string(&mut body)
        .map_err(|e| FetchError::Http(e.to_string()))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| FetchError::Http(e.to_string()))?;
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for item in arr {
        let repo = item
            .get("modelId")
            .or_else(|| item.get("id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if repo.is_empty() {
            continue;
        }
        out.push(HfModelHit {
            repo,
            downloads: item.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
            likes: item.get("likes").and_then(|x| x.as_u64()).unwrap_or(0),
            pipeline: item
                .get("pipeline_tag")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    Ok(out)
}

fn urlencoding_minimal(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => o.push(c),
            ' ' => o.push_str("%20"),
            _ => {
                for b in c.to_string().as_bytes() {
                    o.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    o
}

fn seed_query_for_vram(vram_mb: u64) -> (&'static str, &'static str) {
    // (HF search query, preferred filename fragment)
    if vram_mb >= 16_000 {
        ("Qwen2.5-7B Instruct GGUF", "Q4_K_M")
    } else if vram_mb >= 8_000 {
        ("Qwen2.5-3B Instruct GGUF", "Q4_K_M")
    } else if vram_mb >= 4_000 {
        ("Qwen2.5-1.5B Instruct GGUF", "Q4_K_M")
    } else if vram_mb > 0 {
        ("Qwen2.5-0.5B Instruct GGUF", "Q4_K_M")
    } else {
        ("TinyLlama GGUF Q4", "Q4_K_M")
    }
}

/// Capacity → quant → busca HF → plano recomendado (sem download).
pub fn plan_models_for_capacity(cap: &NodeCapacity) -> ModelPlan {
    let capacity = suggest_for_capacity(cap);
    let (q, file_frag) = seed_query_for_vram(cap.vram_free_mb);
    let search_query = format!("{} {}", q, capacity.quant);
    let candidates = search_hf_gguf(&search_query, 8).unwrap_or_default();
    let (recommended_repo, note) = if let Some(top) = candidates.first() {
        (
            top.repo.clone(),
            format!(
                "top HF downloads={} · quant={} · ngl={}",
                top.downloads, capacity.quant, capacity.suggest_ngl
            ),
        )
    } else {
        (
            "Qwen/Qwen2.5-1.5B-Instruct-GGUF".into(),
            "HF search empty/offline — fallback repo canónico".into(),
        )
    };
    let recommended_file = format!(
        "qwen2.5-*-{}.gguf",
        capacity.quant.to_ascii_lowercase().replace('_', "-")
    );
    let _ = file_frag;
    ModelPlan {
        capacity,
        search_query,
        candidates,
        recommended_repo,
        recommended_file,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity::NodeCapacity;
    use tempfile::tempdir;

    #[test]
    fn suggest_low_vram() {
        let c = NodeCapacity {
            vram_free_mb: 5000,
            vendor_hint: "nvidia".into(),
            ..Default::default()
        };
        let s = suggest_for_capacity(&c);
        assert_eq!(s.quant, "Q3_K_M");
    }

    #[test]
    fn hf_url() {
        let u = hf_resolve_url("org/model", "model.Q4_K_M.gguf");
        assert!(u.contains("huggingface.co/org/model/resolve/main/model.Q4_K_M.gguf"));
    }

    #[test]
    fn safe_filename_rejects_traversal() {
        assert_eq!(safe_filename("model.gguf").unwrap(), "model.gguf");
        for bad in ["", ".", "..", "../evil", "a/b", "a\\b", "/etc/passwd"] {
            assert!(safe_filename(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn fetch_rejects_traversal_before_network() {
        let dir = tempdir().unwrap();
        let err = fetch_to_dir("http://127.0.0.1:1/x", dir.path(), "../evil").unwrap_err();
        assert!(matches!(err, FetchError::BadArgs(_)));
    }

    #[test]
    fn gguf_magic_is_checked() {
        let dir = tempdir().unwrap();
        let bad = dir.path().join("bad.gguf");
        std::fs::write(&bad, b"NOPE").unwrap();
        assert!(verify_gguf_magic(&bad).is_err());

        let good = dir.path().join("good.gguf");
        std::fs::write(&good, b"GGUF....").unwrap();
        assert!(verify_gguf_magic(&good).is_ok());

        let other = dir.path().join("firmware.bin");
        std::fs::write(&other, b"NOPE").unwrap();
        assert!(verify_gguf_magic(&other).is_ok());
    }
}
