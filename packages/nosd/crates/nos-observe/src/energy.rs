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

/// Potência instantânea + temperatura da GPU via um único `nvidia-smi`.
///
/// Ausência do binário/driver = ambos `None` (não é erro).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct NvmlInfo {
    pub watts: Option<f64>,
    pub temp_c: Option<f64>,
}

pub fn read_nvml_info() -> NvmlInfo {
    let Some(smi) = nos_core::nvidia_smi_path() else {
        return NvmlInfo::default();
    };
    let out = match std::process::Command::new(smi)
        .args([
            "--query-gpu=power.draw,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return NvmlInfo::default(),
    };
    if !out.status.success() {
        return NvmlInfo::default();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut watts = 0.0f64;
    let mut seen_w = false;
    let mut temp: Option<f64> = None;
    for line in text.lines() {
        let mut parts = line.trim().split(',');
        if let Some(w) = parts.next().and_then(|v| v.trim().parse::<f64>().ok()) {
            if w.is_finite() && w >= 0.0 {
                watts += w;
                seen_w = true;
            }
        }
        if let Some(t) = parts.next().and_then(|v| v.trim().parse::<f64>().ok()) {
            if t.is_finite() && (-50.0..=150.0).contains(&t) {
                temp = Some(temp.map_or(t, |cur| cur.max(t)));
            }
        }
    }
    NvmlInfo {
        watts: if seen_w { Some(watts) } else { None },
        temp_c: temp,
    }
}

/// Compat: watts agregados de todas as GPUs.
pub fn read_nvml_watts() -> Option<f64> {
    read_nvml_info().watts
}

/// Temperaturas/fans lidos de `/sys/class/hwmon` (Linux); vazio noutros SO.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThermalSample {
    pub cpu_c: Option<f64>,
    pub gpu_c: Option<f64>,
    pub max_c: Option<f64>,
    pub fan_rpm: Option<u32>,
    pub fan_pct: Option<f64>,
}

#[cfg(target_os = "linux")]
fn read_sys_num(path: &std::path::Path) -> Option<f64> {
    std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
}

/// Lê hwmon (temp/fan/pwm). Não spawna processos; no Windows devolve vazio.
pub fn read_thermal() -> ThermalSample {
    #[cfg(target_os = "linux")]
    {
        read_thermal_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        ThermalSample::default()
    }
}

#[cfg(target_os = "linux")]
fn read_thermal_linux() -> ThermalSample {
    let root = std::path::Path::new("/sys/class/hwmon");
    let Ok(chips) = std::fs::read_dir(root) else {
        return ThermalSample::default();
    };
    let mut out = ThermalSample::default();
    let mut max_temp = f64::NEG_INFINITY;
    let mut rpm: Option<u32> = None;
    let mut pwm: Option<u32> = None;
    for chip in chips.flatten() {
        let path = chip.path();
        let name = std::fs::read_to_string(path.join("name"))
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let is_cpu = name.contains("coretemp")
            || name.contains("k10temp")
            || name.contains("zenpower")
            || name.contains("cpu");
        let is_gpu = name.contains("amdgpu")
            || name.contains("nouveau")
            || name.contains("nvidia")
            || name.contains("radeon")
            || name.contains("gpu");
        let Ok(files) = std::fs::read_dir(&path) else {
            continue;
        };
        for f in files.flatten() {
            let fname = f.file_name().to_string_lossy().to_string();
            let fpath = f.path();
            if fname.starts_with("temp") && fname.ends_with("_input") {
                if let Some(mc) = read_sys_num(&fpath) {
                    let c = mc / 1000.0;
                    if c.is_finite() {
                        max_temp = max_temp.max(c);
                        if is_cpu {
                            out.cpu_c = Some(out.cpu_c.map_or(c, |cur| cur.max(c)));
                        } else if is_gpu {
                            out.gpu_c = Some(out.gpu_c.map_or(c, |cur| cur.max(c)));
                        }
                    }
                }
            } else if fname.starts_with("fan") && fname.ends_with("_input") {
                if let Some(v) = read_sys_num(&fpath) {
                    if v.is_finite() && v >= 0.0 {
                        let v = v as u32;
                        rpm = Some(rpm.map_or(v, |cur| cur.max(v)));
                    }
                }
            } else if fname.starts_with("pwm") && !fname.ends_with("_enable") {
                if let Some(v) = read_sys_num(&fpath) {
                    if v.is_finite() && (0.0..=255.0).contains(&v) {
                        let v = v as u32;
                        pwm = Some(pwm.map_or(v, |cur| cur.max(v)));
                    }
                }
            }
        }
    }
    if max_temp.is_finite() {
        out.max_c = Some(max_temp);
    }
    out.fan_rpm = rpm;
    out.fan_pct = pwm.map(|p| (p as f64 / 255.0 * 100.0).clamp(0.0, 100.0));
    out
}

/// Recomendação **advisory** (o nosd não controla ventoinhas):
/// `fan_more_critical` ≥88 °C, `fan_more` ≥78 °C, `fan_less` ≤55 °C com
/// ventoinha já acima de 35 %, `hold` no meio, `no_sensor` sem temperatura.
pub fn thermal_hint(max_c: Option<f64>, fan_pct: Option<f64>) -> String {
    match max_c {
        None => "no_sensor",
        Some(t) if t >= 88.0 => "fan_more_critical",
        Some(t) if t >= 78.0 => "fan_more",
        Some(t) if t <= 55.0 && fan_pct.map(|p| p >= 35.0).unwrap_or(false) => "fan_less",
        Some(_) => "hold",
    }
    .to_string()
}

/// Energia de um componente (CPU package ou GPU).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyPart {
    pub watts: Option<f64>,
    pub joules: f64,
    pub total_j: f64,
    pub source: String,
    pub temp_c: Option<f64>,
}

/// Termal/fan mostrado no painel (recomendação, nunca actuação).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalPart {
    pub cpu_c: Option<f64>,
    pub gpu_c: Option<f64>,
    pub max_c: Option<f64>,
    pub fan_rpm: Option<u32>,
    pub fan_pct: Option<f64>,
    pub hint: String,
}

/// V10 HUD: CPU e GPU separados + termal. O agregado `EnergyDelta` continua
/// a ser a fonte do custo/turno; este medidor é independente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergySplit {
    pub cpu: EnergyPart,
    pub gpu: EnergyPart,
    pub thermal: ThermalPart,
    pub joules: f64,
    pub watts: Option<f64>,
    pub total_j: f64,
    pub ts_ms: u64,
}

#[derive(Debug, Clone)]
pub struct EnergySplitMeter {
    last_uj: Option<u64>,
    last_ms: Option<u64>,
    cpu_total_j: f64,
    gpu_total_j: f64,
}

impl Default for EnergySplitMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl EnergySplitMeter {
    pub fn new() -> Self {
        Self {
            last_uj: None,
            last_ms: None,
            cpu_total_j: 0.0,
            gpu_total_j: 0.0,
        }
    }

    pub fn sample(&mut self) -> EnergySplit {
        let uj = read_rapl_uj();
        let nvml = read_nvml_info();
        let thermal = read_thermal();
        self.sample_with(uj, nvml, thermal, now_ms())
    }

    /// Injeta leituras (testes determinísticos).
    pub fn sample_with(
        &mut self,
        uj: Option<u64>,
        nvml: NvmlInfo,
        mut thermal: ThermalSample,
        now_ms: u64,
    ) -> EnergySplit {
        let dt_ms = self.last_ms.map(|t| now_ms.saturating_sub(t)).unwrap_or(0);
        let mut cpu_j = 0.0f64;
        let mut cpu_source = "absent";
        if let Some(cur) = uj {
            cpu_source = "rapl";
            if let Some(prev) = self.last_uj {
                if cur >= prev {
                    cpu_j = (cur - prev) as f64 / 1_000_000.0;
                } else {
                    // Wrap do contador: zero, nunca joules inventados.
                    cpu_source = "rapl_wrap";
                }
            }
        }
        let cpu_w = if dt_ms > 0 && cpu_j > 0.0 {
            Some(cpu_j / (dt_ms as f64 / 1000.0))
        } else {
            None
        };

        let mut gpu_j = 0.0f64;
        let gpu_source = if nvml.watts.is_some() {
            "nvml"
        } else {
            "absent"
        };
        if let (Some(w), true) = (nvml.watts, dt_ms > 0) {
            gpu_j = w * (dt_ms as f64) / 1000.0;
        }
        if thermal.gpu_c.is_none() {
            thermal.gpu_c = nvml.temp_c;
        }
        thermal.max_c = match (thermal.max_c, nvml.temp_c) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let hint = thermal_hint(thermal.max_c, thermal.fan_pct);

        if !cpu_j.is_finite() || cpu_j < 0.0 {
            cpu_j = 0.0;
        }
        if !gpu_j.is_finite() || gpu_j < 0.0 {
            gpu_j = 0.0;
        }
        self.cpu_total_j += cpu_j;
        self.gpu_total_j += gpu_j;
        self.last_uj = if cpu_source == "rapl_wrap" { None } else { uj };
        self.last_ms = Some(now_ms);

        let joules = cpu_j + gpu_j;
        let total_j = self.cpu_total_j + self.gpu_total_j;
        let watts = if dt_ms > 0 && joules > 0.0 {
            Some(joules / (dt_ms as f64 / 1000.0))
        } else {
            None
        };
        EnergySplit {
            cpu: EnergyPart {
                watts: cpu_w,
                joules: cpu_j,
                total_j: self.cpu_total_j,
                source: cpu_source.to_string(),
                temp_c: thermal.cpu_c,
            },
            gpu: EnergyPart {
                watts: nvml.watts,
                joules: gpu_j,
                total_j: self.gpu_total_j,
                source: gpu_source.to_string(),
                temp_c: thermal.gpu_c,
            },
            thermal: ThermalPart {
                cpu_c: thermal.cpu_c,
                gpu_c: thermal.gpu_c,
                max_c: thermal.max_c,
                fan_rpm: thermal.fan_rpm,
                fan_pct: thermal.fan_pct,
                hint,
            },
            joules,
            watts,
            total_j,
            ts_ms: now_ms,
        }
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
        let _ = read_thermal();
        let _ = read_nvml_info();
    }

    #[test]
    fn split_separates_cpu_and_gpu() {
        let mut m = EnergySplitMeter::new();
        let a = m.sample_with(
            Some(1_000_000),
            NvmlInfo {
                watts: Some(10.0),
                temp_c: Some(60.0),
            },
            ThermalSample {
                cpu_c: Some(55.0),
                ..Default::default()
            },
            1_000,
        );
        assert_eq!(a.cpu.joules, 0.0, "primeira amostra CPU é base");
        assert_eq!(a.gpu.joules, 0.0, "dt=0 não inventa joules GPU");
        assert_eq!(a.gpu.watts, Some(10.0));
        assert_eq!(a.cpu.temp_c, Some(55.0));
        assert_eq!(a.gpu.temp_c, Some(60.0));

        let b = m.sample_with(
            Some(3_000_000),
            NvmlInfo {
                watts: Some(10.0),
                temp_c: Some(61.0),
            },
            ThermalSample {
                cpu_c: Some(56.0),
                ..Default::default()
            },
            2_000,
        );
        assert!((b.cpu.joules - 2.0).abs() < 1e-9);
        assert!((b.gpu.joules - 10.0).abs() < 1e-9);
        assert!((b.joules - 12.0).abs() < 1e-9);
        assert_eq!(b.cpu.source, "rapl");
        assert_eq!(b.gpu.source, "nvml");
        assert_eq!(b.thermal.hint, "hold");
        assert!((b.total_j - 12.0).abs() < 1e-9);
    }

    #[test]
    fn split_never_invents_when_absent() {
        let mut m = EnergySplitMeter::new();
        let _ = m.sample_with(None, NvmlInfo::default(), ThermalSample::default(), 1_000);
        let b = m.sample_with(None, NvmlInfo::default(), ThermalSample::default(), 2_000);
        assert_eq!(b.joules, 0.0);
        assert_eq!(b.cpu.source, "absent");
        assert_eq!(b.gpu.source, "absent");
        assert_eq!(b.total_j, 0.0);
        assert_eq!(b.thermal.hint, "no_sensor");
    }

    #[test]
    fn thermal_hint_escalates_by_temperature() {
        assert_eq!(thermal_hint(None, None), "no_sensor");
        assert_eq!(thermal_hint(Some(40.0), Some(10.0)), "hold");
        assert_eq!(thermal_hint(Some(50.0), Some(60.0)), "fan_less");
        assert_eq!(thermal_hint(Some(79.0), Some(40.0)), "fan_more");
        assert_eq!(thermal_hint(Some(90.0), Some(100.0)), "fan_more_critical");
    }
}
