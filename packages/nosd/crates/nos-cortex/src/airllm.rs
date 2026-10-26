//! AirLLM layer-stream plan + executor thin (P.5).
//! Honesty: executor simula passes em host; llama-server layer mmap real = residual metal.

use serde::{Deserialize, Serialize};

use crate::fit_policy::{fit_model, FitVerdict};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AirLlmPlan {
    /// Stream por camadas (vs full mmap).
    pub layer_stream: bool,
    pub layers_per_pass: u32,
    pub estimated_passes: u32,
    pub need_mb: u64,
    pub avail_mb: u64,
    pub reason: String,
}

impl AirLlmPlan {
    /// true = TooTight path (stream); caller decide se executa `run_layer_stream`.
    pub fn plan_only(&self, verdict: FitVerdict) -> bool {
        self.layer_stream && matches!(verdict, FitVerdict::Tight | FitVerdict::TooTight)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AirLlmPass {
    pub pass_idx: u32,
    pub layer_lo: u32,
    pub layer_hi: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AirLlmStreamReport {
    pub passes: Vec<AirLlmPass>,
    pub ok: bool,
    pub note: String,
}

/// Se TooTight/Tight, sugere stream por N camadas (default 4).
pub fn plan_airllm(
    need_mb: u64,
    avail_ram_mb: u64,
    avail_vram_mb: u64,
    n_layers: u32,
) -> AirLlmPlan {
    let fit = fit_model(need_mb, avail_ram_mb, avail_vram_mb);
    let avail_mb = avail_ram_mb.saturating_add(avail_vram_mb);
    let n_layers = n_layers.max(1);
    match fit.verdict {
        FitVerdict::Fits => AirLlmPlan {
            layer_stream: false,
            layers_per_pass: n_layers,
            estimated_passes: 1,
            need_mb,
            avail_mb,
            reason: "fits — full load".into(),
        },
        FitVerdict::Tight | FitVerdict::TooTight => {
            let per = 4u32.min(n_layers).max(1);
            let passes = n_layers.div_ceil(per);
            AirLlmPlan {
                layer_stream: true,
                layers_per_pass: per,
                estimated_passes: passes,
                need_mb,
                avail_mb,
                reason: format!(
                    "{:?} need={}MB pool={}MB → layer-stream {per}x{passes}",
                    fit.verdict, need_mb, avail_mb
                ),
            }
        }
    }
}

/// Executor thin: materializa passes (sem pesos). `on_pass` pode logar / HITL cancel.
pub fn run_layer_stream<F>(plan: &AirLlmPlan, n_layers: u32, mut on_pass: F) -> AirLlmStreamReport
where
    F: FnMut(&AirLlmPass) -> bool,
{
    if !plan.layer_stream {
        return AirLlmStreamReport {
            passes: vec![AirLlmPass {
                pass_idx: 0,
                layer_lo: 0,
                layer_hi: n_layers.saturating_sub(1),
            }],
            ok: true,
            note: "full load — no stream".into(),
        };
    }
    let n_layers = n_layers.max(1);
    let per = plan.layers_per_pass.max(1);
    let mut passes = Vec::new();
    let mut lo = 0u32;
    let mut idx = 0u32;
    while lo < n_layers {
        let hi = (lo + per - 1).min(n_layers - 1);
        let p = AirLlmPass {
            pass_idx: idx,
            layer_lo: lo,
            layer_hi: hi,
        };
        if !on_pass(&p) {
            return AirLlmStreamReport {
                passes,
                ok: false,
                note: format!("cancelled at pass {idx}"),
            };
        }
        passes.push(p);
        lo = hi.saturating_add(1);
        idx = idx.saturating_add(1);
    }
    AirLlmStreamReport {
        note: format!(
            "streamed {} passes (thin host — no weight mmap)",
            passes.len()
        ),
        ok: true,
        passes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_tight_enables_stream() {
        let p = plan_airllm(8000, 1000, 500, 32);
        assert!(p.layer_stream);
        assert!(p.estimated_passes > 1);
        assert!(p.plan_only(FitVerdict::TooTight));
    }

    #[test]
    fn fits_disables() {
        let p = plan_airllm(100, 8000, 8000, 32);
        assert!(!p.layer_stream);
    }

    #[test]
    fn run_stream_counts_passes() {
        let p = plan_airllm(8000, 1000, 500, 32);
        let r = run_layer_stream(&p, 32, |_| true);
        assert!(r.ok);
        assert_eq!(r.passes.len() as u32, p.estimated_passes);
        assert_eq!(r.passes[0].layer_lo, 0);
        assert_eq!(r.passes.last().unwrap().layer_hi, 31);
    }

    #[test]
    fn run_stream_cancel() {
        let p = plan_airllm(8000, 1000, 500, 32);
        let r = run_layer_stream(&p, 32, |pass| pass.pass_idx < 1);
        assert!(!r.ok);
        assert_eq!(r.passes.len(), 1);
    }
}
