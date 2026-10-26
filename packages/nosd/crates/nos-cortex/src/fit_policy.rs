//! FitPolicy / evict (T.3) — TooTight → AirLLM/evict honesty, never Silent miss.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitVerdict {
    Fits,
    Tight,
    TooTight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitReport {
    pub verdict: FitVerdict,
    pub need_mb: u64,
    pub avail_ram_mb: u64,
    pub avail_vram_mb: u64,
    pub suggest_airllm: bool,
    pub suggest_evict: bool,
}

pub fn fit_model(need_mb: u64, avail_ram_mb: u64, avail_vram_mb: u64) -> FitReport {
    let pool = avail_ram_mb.saturating_add(avail_vram_mb);
    let verdict = if need_mb + 512 <= pool {
        FitVerdict::Fits
    } else if need_mb <= pool {
        FitVerdict::Tight
    } else {
        FitVerdict::TooTight
    };
    FitReport {
        verdict,
        need_mb,
        avail_ram_mb,
        avail_vram_mb,
        suggest_airllm: matches!(verdict, FitVerdict::TooTight | FitVerdict::Tight),
        suggest_evict: matches!(verdict, FitVerdict::TooTight),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_tight_suggests_airllm() {
        let r = fit_model(8000, 2000, 2000);
        assert_eq!(r.verdict, FitVerdict::TooTight);
        assert!(r.suggest_airllm);
        assert!(r.suggest_evict);
    }

    #[test]
    fn fits_ok() {
        let r = fit_model(500, 8000, 8000);
        assert_eq!(r.verdict, FitVerdict::Fits);
        assert!(!r.suggest_airllm);
    }
}
