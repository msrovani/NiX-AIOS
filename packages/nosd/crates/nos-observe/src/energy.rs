//! V1 (plano vampiro) — energia por janela, medida ou honestamente estimada.
//!
//! Origem da ideia: `openjarvis/telemetry/{energy_rapl,energy_nvidia,itl}.rs`.
//!
//! **Contrato de honestidade:** cada amostra traz `source`:
//! `rapl` (contador do kernel), `nvml` (`nvidia-smi`), `tdp_estimate`
//! (`NOSD_TDP_W` × tempo — explicitamente uma estimativa) ou `absent`
//! (sem fonte: **zero** joules, nunca um número inventado).
//!
//! Leitura RAPL (`/sys/class/powercap/intel-rapl:0/energy_uj`) costuma exigir
//! root ou regra udev; quando falha, cai para NVML e depois para `absent`.
//! O contador é monotónico: uma descida (wrap do kernel) **não** é convertida
//! em joules — é tratada como `rapl_wrap` e reinicia a base.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Candidatos de contador RAPL, por ordem de preferência.
pub const RAPL_CANDIDATES: &[&str] = &[
    "/sys/class/powercap/intel-rapl:0/energy_uj",
    "/sys/class/powercap/intel-rapl:0:0/energy_uj",
    "/sys/class/powercap/amd-rapl:0/energy_uj",
];

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Lê o primeiro contador RAPL disponível (microjoules).
pub fn read_rapl_uj() -> Option<u64> {
    for path in RAPL_CANDIDATES {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(v) = text.trim().parse::<u64>() {
                return Some(v);
            }
        }
    }
    None
}

/// Soma `power.draw` de todas as GPUs via `nvidia-smi` (watts).
///
/// Ausência do binário ou de driver = `None` (não é erro).
pub fn read_nvml_watts() -> Option<f64> {
    let smi = nos_core::nvidia_smi_path()?;
    let out = std::process::Command::new(smi)
        .args(["--query-gpu=power.draw", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut total = 0.0f64;
    let mut seen = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(w) = line.parse::<f64>() {
            if w.is_finite() && w >= 0.0 {
                total += w;
                seen = true;
            }
        }
    }
    if seen {
        Some(total)
    } else {
        None
    }
}

/// Amostra de energia numa janela (desde a amostra anterior).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyDelta {
    /// Joules consumidos desde a amostra anterior.
    pub joules: f64,
    /// Joules acumulados desde a criação do medidor.
    pub joules_total: f64,
    /// Potência média na janela (W) — `None` se a janela for 0 ms.
    pub watts: Option<f64>,
    /// `rapl` | `rapl_wrap` | `nvml` | `tdp_estimate` | `absent`
    pub source: String,
    /// O que foi medido: `cpu_package` | `gpu` | `machine_estimate` | `none`.
    ///
    /// **Crítico:** RAPL mede o *package* da CPU; NVML mede a *GPU*. Nenhum
    /// dos dois é o consumo da máquina — o custo local é um **limite
    /// inferior** excepto com `tdp_estimate` (que é uma estimativa).
    pub scope: String,
    pub ts_ms: u64,
}

/// Medidor monotónico. Barato: sysfs; NVML só quando não há RAPL.
#[derive(Debug, Clone)]
pub struct EnergyMeter {
    last_uj: Option<u64>,
    last_ms: Option<u64>,
    total_j: f64,
    tdp_w: Option<f64>,
}

impl Default for EnergyMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl EnergyMeter {
    /// `NOSD_TDP_W` habilita `tdp_estimate` (nunca por omissão).
    pub fn new() -> Self {
        let tdp_w = std::env::var("NOSD_TDP_W")
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0);
        Self {
            last_uj: None,
            last_ms: None,
            total_j: 0.0,
            tdp_w,
        }
    }

    /// Injeta um TDP (usado nos testes e por config explícita).
    pub fn with_tdp(mut self, watts: f64) -> Self {
        self.tdp_w = if watts.is_finite() && watts > 0.0 {
            Some(watts)
        } else {
            None
        };
        self
    }

    pub fn prime(&mut self) {
        // Base da janela: só sysfs (barato). Não spawna `nvidia-smi`.
        if let Some(uj) = read_rapl_uj() {
            self.last_uj = Some(uj);
        }
        self.last_ms = Some(now_ms());
    }

    pub fn total_joules(&self) -> f64 {
        self.total_j
    }

    /// Amostra real: RAPL → NVML → TDP → absent.
    pub fn sample(&mut self) -> EnergyDelta {
        let uj = read_rapl_uj();
        let nvml = if uj.is_none() {
            read_nvml_watts()
        } else {
            None
        };
        self.sample_with(uj, nvml, now_ms())
    }

    /// Amostra determinística (testes): recebe as leituras do host.
    pub fn sample_with(
        &mut self,
        uj: Option<u64>,
        nvml_w: Option<f64>,
        now_ms: u64,
    ) -> EnergyDelta {
        let dt_ms = self.last_ms.map(|t| now_ms.saturating_sub(t)).unwrap_or(0);
        let mut source = "absent";
        let mut joules = 0.0f64;
        let mut wrap = false;

        if let Some(cur) = uj {
            // Com contador presente a fonte é sempre RAPL (a 1.ª amostra é base).
            source = "rapl";
            if let Some(prev) = self.last_uj {
                if cur >= prev {
                    joules = (cur - prev) as f64 / 1_000_000.0;
                } else {
                    // Contador reiniciou: não fabricar joules.
                    source = "rapl_wrap";
                    wrap = true;
                }
            }
        } else if let (Some(w), true) = (nvml_w, dt_ms > 0) {
            joules = w * (dt_ms as f64) / 1000.0;
            source = "nvml";
        } else if let (Some(w), true) = (self.tdp_w, dt_ms > 0) {
            joules = w * (dt_ms as f64) / 1000.0;
            source = "tdp_estimate";
        }

        if !joules.is_finite() || joules < 0.0 {
            joules = 0.0;
            source = "absent";
        }
        self.total_j += joules;
        // Sem contador nesta amostra → limpar a base (um reaparecimento conta
        // como nova base, não como delta gigante).
        self.last_uj = if wrap { None } else { uj };
        self.last_ms = Some(now_ms);

        EnergyDelta {
            joules,
            joules_total: self.total_j,
            watts: if dt_ms > 0 && joules > 0.0 {
                Some(joules / (dt_ms as f64 / 1000.0))
            } else {
                None
            },
            source: source.to_string(),
            scope: match source {
                "rapl" | "rapl_wrap" => "cpu_package",
                "nvml" => "gpu",
                "tdp_estimate" => "machine_estimate",
                _ => "none",
            }
            .to_string(),
            ts_ms: now_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapl_delta_is_measured() {
        let mut m = EnergyMeter::new();
        let a = m.sample_with(Some(1_000_000), None, 1_000);
        assert_eq!(a.joules, 0.0, "primeira amostra é só base");
        assert_eq!(a.source, "rapl");
        let b = m.sample_with(Some(3_000_000), None, 2_000);
        assert!((b.joules - 2.0).abs() < 1e-9);
        assert!((b.watts.unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(b.scope, "cpu_package");
        assert!((m.total_joules() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn rapl_wrap_never_invents_energy() {
        let mut m = EnergyMeter::new();
        let _ = m.sample_with(Some(10_000), None, 1_000);
        let b = m.sample_with(Some(5), None, 2_000);
        assert_eq!(b.joules, 0.0);
        assert_eq!(b.source, "rapl_wrap");
        assert_eq!(b.scope, "cpu_package");
        assert_eq!(m.total_joules(), 0.0);
    }

    #[test]
    fn nvml_is_used_when_rapl_absent() {
        let mut m = EnergyMeter::new();
        let _ = m.sample_with(None, Some(10.0), 0);
        let b = m.sample_with(None, Some(10.0), 2_000);
        assert!((b.joules - 20.0).abs() < 1e-9);
        assert_eq!(b.source, "nvml");
        assert_eq!(b.scope, "gpu");
        assert!((b.watts.unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn tdp_estimate_is_labelled_and_needs_config() {
        let mut m = EnergyMeter::new().with_tdp(20.0);
        let _ = m.sample_with(None, None, 0);
        let b = m.sample_with(None, None, 1_000);
        assert!((b.joules - 20.0).abs() < 1e-9);
        assert_eq!(b.source, "tdp_estimate");
        assert_eq!(b.scope, "machine_estimate");

        let mut none = EnergyMeter::new().with_tdp(0.0);
        let _ = none.sample_with(None, None, 0);
        let c = none.sample_with(None, None, 1_000);
        assert_eq!(c.joules, 0.0);
        assert_eq!(c.source, "absent");
        assert_eq!(c.scope, "none");
    }

    #[test]
    fn zero_window_has_no_watts() {
        let mut m = EnergyMeter::new();
        let _ = m.sample_with(Some(0), None, 5_000);
        let b = m.sample_with(Some(1_000), None, 5_000);
        assert!(b.watts.is_none());
    }

    #[test]
    fn rapl_reader_is_infallible_off_linux() {
        // Windows/CI: None é resposta válida, não panic.
        let _ = read_rapl_uj();
        let _ = read_nvml_watts();
    }
}
