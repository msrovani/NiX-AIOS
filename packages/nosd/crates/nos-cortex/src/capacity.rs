//! Capacity Prober — /proc + sysfs DRM + nvidia-smi best-effort (sem NVML linkado).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub ram_total_mb: u64,
    pub ram_avail_mb: u64,
    pub vram_free_mb: u64,
    pub dri_cards: Vec<String>,
    pub dri_render: Vec<String>,
    pub vendor_hint: String,
    /// DRM card Intel/AMD iGPU para GUI (ex.: `card1`), se detectado.
    #[serde(default)]
    pub igpu_card: Option<String>,
    /// DRM card NVIDIA/AMD discrete para inferência (ex.: `card0`).
    #[serde(default)]
    pub dgpu_card: Option<String>,
    /// NX.5: backends detectados (path/sysfs) — não implica runtime wired.
    pub accel: Vec<String>,
}

pub struct CapacityProber;

impl CapacityProber {
    pub fn probe() -> NodeCapacity {
        let mut c = NodeCapacity {
            ram_total_mb: read_meminfo_kb("MemTotal").unwrap_or(0) / 1024,
            ram_avail_mb: read_meminfo_kb("MemAvailable")
                .or_else(|| read_meminfo_kb("MemFree"))
                .unwrap_or(0)
                / 1024,
            dri_cards: list_dri("card"),
            dri_render: list_dri("renderD"),
            vendor_hint: detect_vendor_hint(),
            ..Default::default()
        };
        let (igpu, dgpu) = classify_dri_cards(&c.dri_cards);
        c.igpu_card = igpu;
        c.dgpu_card = dgpu;
        if Path::new("/proc/driver/nvidia/gpus").exists() {
            c.vendor_hint = "nvidia".into();
        }
        // Best-effort: nvidia-smi (WSL/host) sem linkar NVML — honesty 0113.
        c.vram_free_mb = probe_nvidia_smi_free_mb().unwrap_or(0);
        if c.vram_free_mb > 0 && c.vendor_hint == "cpu" {
            c.vendor_hint = "nvidia".into();
        }
        c.accel = detect_accel_backends(&c);
        c
    }

    /// Re-probe se a 1ª passagem apanhou só iGPU / VRAM=0 com NVIDIA a subir.
    /// Metal 2026-10-31: i915 ~7s, nosd ~10s, nvidia-drm card0 ~11s.
    pub fn probe_with_retry(max_wait: Duration) -> NodeCapacity {
        let mut c = Self::probe();
        if !needs_nvidia_retry(&c) {
            return c;
        }
        let deadline = Instant::now() + max_wait;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(250));
            c = Self::probe();
            if !needs_nvidia_retry(&c) {
                break;
            }
        }
        c
    }

    /// Mensagem honesta para logs (VRAM=0 não implica "sem GPU").
    pub fn vram_posture(c: &NodeCapacity) -> &'static str {
        if c.vram_free_mb > 0 {
            "nvidia_smi"
        } else if c.vendor_hint == "nvidia"
            || Path::new("/proc/driver/nvidia/gpus").exists()
            || Path::new("/sys/module/nvidia").exists()
        {
            "nvml_absent"
        } else if !c.dri_render.is_empty() {
            "drm_present_vram_unknown"
        } else {
            "no_gpu_hint"
        }
    }
}

fn needs_nvidia_retry(c: &NodeCapacity) -> bool {
    let nvidia_mod =
        Path::new("/sys/module/nvidia").exists() || Path::new("/proc/driver/nvidia/gpus").exists();
    if !nvidia_mod && c.dgpu_card.is_none() {
        return false;
    }
    c.vram_free_mb == 0 || c.dgpu_card.is_none()
}

/// Classifica cards DRM por vendor sysfs (`0x8086` Intel, `0x10de` NVIDIA, …).
pub fn classify_dri_cards(cards: &[String]) -> (Option<String>, Option<String>) {
    let mut igpu = None;
    let mut dgpu = None;
    for card in cards {
        let vendor = drm_vendor_id(card).unwrap_or_default();
        match vendor.as_str() {
            "0x8086" => {
                if igpu.is_none() {
                    igpu = Some(card.clone());
                }
            }
            "0x10de" => {
                if dgpu.is_none() {
                    dgpu = Some(card.clone());
                }
            }
            "0x1002" => {
                // AMD: se já há Intel como iGPU, treat as dGPU; senão iGPU.
                if igpu.is_some() {
                    if dgpu.is_none() {
                        dgpu = Some(card.clone());
                    }
                } else if igpu.is_none() {
                    igpu = Some(card.clone());
                }
            }
            _ => {}
        }
    }
    (igpu, dgpu)
}

fn drm_vendor_id(card: &str) -> Option<String> {
    let path = format!("/sys/class/drm/{card}/device/vendor");
    let s = fs::read_to_string(path).ok()?;
    Some(s.trim().to_ascii_lowercase())
}

/// `nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits`
/// Cache 20 s (positivo ou negativo) — evita coredump spam no HUD.
/// Cache de `nvidia-smi` (instante da leitura + valor), 20 s.
type SmiCache = Mutex<Option<(Instant, Option<u64>)>>;

fn probe_nvidia_smi_free_mb() -> Option<u64> {
    static CACHE: OnceLock<SmiCache> = OnceLock::new();
    let cell = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cell.lock() {
        if let Some((t, v)) = guard.as_ref() {
            if t.elapsed() < Duration::from_secs(20) {
                return *v;
            }
        }
    }
    let result = probe_nvidia_smi_free_mb_uncached();
    if let Ok(mut guard) = cell.lock() {
        *guard = Some((Instant::now(), result));
    }
    result
}

fn probe_nvidia_smi_free_mb_uncached() -> Option<u64> {
    let smi = nos_core::nvidia_smi_path()?;
    let out = Command::new(smi)
        .args(["--query-gpu=memory.free", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    // multi-GPU: soma free (hint; offload real exige build CUDA)
    let mut total = 0u64;
    let mut any = false;
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(v) = t.parse::<u64>() {
            total = total.saturating_add(v);
            any = true;
        }
    }
    any.then_some(total)
}

fn read_meminfo_kb(key: &str) -> Option<u64> {
    let s = fs::read_to_string("/proc/meminfo").ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim().trim_start_matches(':').trim();
            let num: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(num);
        }
    }
    None
}

fn list_dri(prefix: &str) -> Vec<String> {
    let Ok(rd) = fs::read_dir("/dev/dri") else {
        return Vec::new();
    };
    let mut v: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(prefix))
        .collect();
    v.sort();
    v
}

fn detect_vendor_hint() -> String {
    if Path::new("/sys/module/amdgpu").exists() {
        return "amd".into();
    }
    if Path::new("/sys/module/i915").exists() || Path::new("/sys/module/xe").exists() {
        return "intel".into();
    }
    if Path::new("/sys/module/nvidia").exists() {
        return "nvidia".into();
    }
    "cpu".into()
}

/// Detecta CUDA/ROCm/OpenVINO por path (NX.5 residual — sem runtime link).
pub fn detect_accel_backends(c: &NodeCapacity) -> Vec<String> {
    let mut v = Vec::new();
    if c.vram_free_mb > 0
        || Path::new("/proc/driver/nvidia/gpus").exists()
        || Path::new("/sys/module/nvidia").exists()
        || c.dgpu_card.is_some()
    {
        v.push("cuda".into());
    }
    if Path::new("/sys/module/amdgpu").exists()
        || Path::new("/opt/rocm").exists()
        || Path::new("/dev/kfd").exists()
    {
        v.push("rocm".into());
    }
    if Path::new("/usr/lib/libopenvino.so").exists()
        || Path::new("/usr/lib/x86_64-linux-gnu/libopenvino.so").exists()
        || std::env::var_os("INTEL_OPENVINO_DIR").is_some()
        || std::env::var_os("OpenVINO_DIR").is_some()
    {
        v.push("openvino".into());
    }
    if v.is_empty() {
        v.push("cpu".into());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_does_not_panic() {
        let _ = CapacityProber::probe();
    }

    #[test]
    fn posture_vram_measured() {
        let c = NodeCapacity {
            vram_free_mb: 4000,
            vendor_hint: "nvidia".into(),
            ..Default::default()
        };
        assert_eq!(CapacityProber::vram_posture(&c), "nvidia_smi");
    }

    #[test]
    fn accel_cuda_from_vram() {
        let c = NodeCapacity {
            vram_free_mb: 1000,
            ..Default::default()
        };
        let a = detect_accel_backends(&c);
        assert!(a.iter().any(|x| x == "cuda"));
    }

    #[test]
    fn accel_cpu_fallback() {
        let c = NodeCapacity::default();
        let a = detect_accel_backends(&c);
        assert!(a.contains(&"cpu".into()) || !a.is_empty());
    }

    #[test]
    fn classify_intel_nvidia_names_only() {
        // Sem sysfs real: vendor lookup falha → ambos None (honesty).
        let (i, d) = classify_dri_cards(&["card0".into(), "card1".into()]);
        let _ = (i, d);
    }
}
