//! WWW / Browser Agent suite (#307 / P.11) — CapGate NET; HITL para cage.

use nos_core::{CapBits, CapGate};
use serde::{Deserialize, Serialize};
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WwwAction {
    pub kind: WwwKind,
    pub url: String,
    pub hitl_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WwwKind {
    Fetch,
    Search,
    Rss,
    Download,
}

#[derive(Debug, Error)]
pub enum WwwError {
    #[error("cap: {0}")]
    Cap(&'static str),
    #[error("hitl required")]
    HitlRequired,
    #[error("http: {0}")]
    Http(String),
}

/// Planeia acção web — download/cage sempre HITL.
pub fn plan_www(kind: WwwKind, url: &str) -> WwwAction {
    let hitl_required = matches!(kind, WwwKind::Download) || url.contains("javascript:");
    WwwAction {
        kind,
        url: url.into(),
        hitl_required,
    }
}

/// Executa GET quando `CapGate` inclui NET e acção não exige HITL.
pub fn fetch_if_allowed(
    gate: &CapGate,
    cap_gen: u64,
    action: &WwwAction,
) -> Result<Vec<u8>, WwwError> {
    if action.hitl_required {
        return Err(WwwError::HitlRequired);
    }
    gate.allow(CapBits::NET, cap_gen).map_err(WwwError::Cap)?;
    if !action.url.starts_with("http://") && !action.url.starts_with("https://") {
        return Err(WwwError::Http("url must be http(s)".into()));
    }
    let mut reader = ureq::get(&action.url)
        .call()
        .map_err(|e| WwwError::Http(e.to_string()))?
        .into_reader();
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| WwwError::Http(e.to_string()))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nos_core::CapBits;

    #[test]
    fn fetch_no_hitl() {
        let a = plan_www(WwwKind::Fetch, "https://example.com");
        assert!(!a.hitl_required);
    }

    #[test]
    fn download_hitl() {
        let a = plan_www(WwwKind::Download, "https://example.com/a.bin");
        assert!(a.hitl_required);
    }

    #[test]
    fn fetch_denied_without_net_cap() {
        let a = plan_www(WwwKind::Fetch, "https://example.com");
        let g = CapGate::new(CapBits::INFER, 1);
        assert!(matches!(
            fetch_if_allowed(&g, 1, &a),
            Err(WwwError::Cap(_))
        ));
    }
}
