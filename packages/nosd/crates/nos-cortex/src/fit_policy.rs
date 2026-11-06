//! FitPolicy / evict (T.3) — TooTight → AirLLM/evict honesty, never Silent miss.
//!
//! C2.1: **VRAM e RAM nunca se somam.** Um modelo cabe numa placa (`basis=vram`),
//! ou faz spill declarado para RAM (`basis=spill`, S1/CPU), ou não cabe
//! (`basis=cpu` = TooTight). O "pool" antigo (`ram_avail + vram_free`)
//! transformava qualquer 4B em 6 GiB de VRAM + 12 GiB de RAM em `Fits` — foi
//! assim que o motor pediu VRAM que não existia.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitVerdict {
    Fits,
    Tight,
    TooTight,
}

/// Onde o veredicto se apoia (C2.1: nada de percentagens sem denominador).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitBasis {
    /// Cabe na VRAM disponível (GPU residente).
    Vram,
    /// Não cabe na VRAM, mas cabe em RAM: spill declarado (CPU/offload parcial).
    Spill,
    /// Não cabe em nenhum dos dois: TooTight.
    Cpu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitReport {
    pub verdict: FitVerdict,
    /// `need_vram_mb + need_ram_mb` (compatibilidade com consumidores antigos).
    pub need_mb: u64,
    pub need_vram_mb: u64,
    pub need_ram_mb: u64,
    pub avail_ram_mb: u64,
    pub avail_vram_mb: u64,
    pub basis: FitBasis,
    pub suggest_airllm: bool,
    pub suggest_evict: bool,
}

/// `allow_spill=false` = caminho CUDA/S2: ou cabe na VRAM, ou não cabe.
/// `allow_spill=true` = S1/CPU: cabe na RAM, mais lento mas honesto.
pub fn fit_model(
    need_vram_mb: u64,
    need_ram_mb: u64,
    avail_ram_mb: u64,
    avail_vram_mb: u64,
    allow_spill: bool,
) -> FitReport {
    let need_mb = need_vram_mb.saturating_add(need_ram_mb);
    let fits_with_headroom = need_vram_mb + 512 <= avail_vram_mb && need_ram_mb <= avail_ram_mb;
    let fits_exactly = need_vram_mb <= avail_vram_mb && need_ram_mb <= avail_ram_mb;
    let fits_in_ram = need_mb + 512 <= avail_ram_mb;

    let (verdict, basis) = if fits_with_headroom {
        (FitVerdict::Fits, FitBasis::Vram)
    } else if fits_exactly {
        // Cabe, mas sem folga para o contexto crescer: jam apertado, não mentir.
        (FitVerdict::Tight, FitBasis::Vram)
    } else if allow_spill && fits_in_ram {
        (FitVerdict::Tight, FitBasis::Spill)
    } else {
        (FitVerdict::TooTight, FitBasis::Cpu)
    };

    FitReport {
        verdict,
        need_mb,
        need_vram_mb,
        need_ram_mb,
        avail_ram_mb,
        avail_vram_mb,
        basis,
        suggest_airllm: matches!(verdict, FitVerdict::TooTight | FitVerdict::Tight),
        suggest_evict: matches!(verdict, FitVerdict::TooTight),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_tight_suggests_airllm() {
        // 8 GB de pesos em 2 GB VRAM e 2 GB RAM, sem spill: não cabe.
        let r = fit_model(8000, 256, 2000, 2000, false);
        assert_eq!(r.verdict, FitVerdict::TooTight);
        assert_eq!(r.basis, FitBasis::Cpu);
        assert!(r.suggest_airllm);
        assert!(r.suggest_evict);
    }

    #[test]
    fn fits_ok() {
        let r = fit_model(500, 128, 8000, 8000, false);
        assert_eq!(r.verdict, FitVerdict::Fits);
        assert_eq!(r.basis, FitBasis::Vram);
        assert!(!r.suggest_airllm);
    }

    #[test]
    fn ram_never_pays_for_vram_on_a_cuda_lane() {
        // O caso do metal: 4B Q6 (~5,4 GB) numa placa de 5,5 GB com 12 GB de
        // RAM. O pool antigo dizia `Fits`; agora cabe sem folga = `Tight`.
        let r = fit_model(5400, 300, 12_288, 5500, false);
        assert_ne!(r.verdict, FitVerdict::Fits, "RAM não é VRAM");
        assert_eq!(r.basis, FitBasis::Vram);

        // 6,5 GB de pesos em 5,5 GB de VRAM: nunca `Fits`, mesmo com RAM a sobrar.
        let r = fit_model(6500, 300, 12_288, 5500, false);
        assert_eq!(r.verdict, FitVerdict::TooTight);
        assert_eq!(r.basis, FitBasis::Cpu);
    }

    #[test]
    fn spill_is_declared_not_hidden() {
        let r = fit_model(6500, 300, 12_288, 5500, true);
        assert_eq!(r.verdict, FitVerdict::Tight);
        assert_eq!(r.basis, FitBasis::Spill);
        assert!(r.suggest_airllm);
        assert!(!r.suggest_evict);
    }

    #[test]
    fn need_mb_is_the_sum_for_old_consumers() {
        let r = fit_model(1000, 250, 8000, 8000, true);
        assert_eq!(r.need_mb, 1250);
    }
}
