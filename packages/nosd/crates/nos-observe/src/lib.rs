//! nos-observe — snapshot read-only do host (CapGate OBSERVE no daemon).
//! sysfs/proc + journal/udev/NM best-effort; Windows → coleções vazias / defaults.
//! Publica no EventBus via `publish_observe` (tópico `OBSERVE_SNAPSHOT`).

#![forbid(unsafe_code)]

pub mod energy;

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::Command;

/// Presença do utilizador / postura do sistema (padrão assistd).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    #[default]
    Active,
    Drowsy,
    Sleep,
}

impl Presence {
    pub fn parse_env(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "active" => Some(Self::Active),
            "drowsy" => Some(Self::Drowsy),
            "sleep" => Some(Self::Sleep),
            _ => None,
        }
    }

    pub fn from_env_or_default() -> Self {
        std::env::var("NOSD_PRESENCE")
            .ok()
            .and_then(|v| Self::parse_env(&v))
            .unwrap_or_default()
    }

    /// Sleep → sugerir libertar VRAM / baixar ngl (NX.5).
    pub fn prefer_vram_release(self) -> bool {
        matches!(self, Self::Sleep | Self::Drowsy)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HwmonReading {
    pub name: String,
    pub temp_mc: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalHint {
    pub source: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdevHint {
    pub source: String,
    /// Nomes de dispositivos sob /sys/class/{net,drm,input} (amostra).
    pub devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NmHint {
    pub source: String,
    pub connectivity: String,
    pub primary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObserveSnapshot {
    pub ts_secs: u64,
    pub hwmon: Vec<HwmonReading>,
    pub drm_cards: Vec<String>,
    pub mem_available_kb: Option<u64>,
    pub net_state: String,
    pub presence: Presence,
    /// true se algum sensor > 85 °C (85000 m°C) — hint para reduzir NGL/carga GPU.
    pub ngl_reduce: bool,
    pub journal: JournalHint,
    pub udev: UdevHint,
    pub nm: NmHint,
    /// Sugestão ngl após thermal + presence (0..=99).
    pub suggest_ngl_cap: u32,
}

const THERMAL_NGL_REDUCE_MC: i64 = 85_000;

pub fn thermal_ngl_reduce(hwmon: &[HwmonReading]) -> bool {
    hwmon.iter().any(|h| h.temp_mc > THERMAL_NGL_REDUCE_MC)
}

/// Cap ngl: thermal → 16; Sleep → 0; Drowsy → 32; senão 99.
pub fn suggest_ngl_cap(presence: Presence, ngl_reduce: bool) -> u32 {
    if presence == Presence::Sleep {
        return 0;
    }
    if ngl_reduce {
        return 16;
    }
    if presence == Presence::Drowsy {
        return 32;
    }
    99
}

pub fn collect_snapshot() -> ObserveSnapshot {
    let hwmon = read_hwmon();
    let ngl_reduce = thermal_ngl_reduce(&hwmon);
    let presence = Presence::from_env_or_default();
    ObserveSnapshot {
        ts_secs: unix_ts_secs(),
        hwmon,
        drm_cards: read_drm_cards(),
        mem_available_kb: read_mem_available_kb(),
        net_state: read_net_state(),
        presence,
        ngl_reduce,
        journal: read_journal_hint(),
        udev: read_udev_hint(),
        nm: read_nm_hint(),
        suggest_ngl_cap: suggest_ngl_cap(presence, ngl_reduce),
    }
}

/// Serializa snapshot para EventBus (caller: Hermes/nosd).
pub fn snapshot_bus_payload(snap: &ObserveSnapshot) -> serde_json::Value {
    serde_json::to_value(snap).unwrap_or(serde_json::json!({ "observe": "serialize_fail" }))
}

pub const TOPIC_OBSERVE_SNAPSHOT: &str = "OBSERVE_SNAPSHOT";

fn unix_ts_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_mem_available_kb() -> Option<u64> {
    #[cfg(unix)]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in text.lines() {
            if line.starts_with("MemAvailable:") {
                let kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok());
                return kb;
            }
        }
        None
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn read_hwmon() -> Vec<HwmonReading> {
    #[cfg(unix)]
    {
        read_hwmon_linux()
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

#[cfg(unix)]
fn read_hwmon_linux() -> Vec<HwmonReading> {
    let root = Path::new("/sys/class/hwmon");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in entries.flatten() {
        let path = ent.path();
        let chip_name = std::fs::read_to_string(path.join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| ent.file_name().to_string_lossy().into_owned());
        let Ok(temps) = std::fs::read_dir(&path) else {
            continue;
        };
        for t in temps.flatten() {
            let fname = t.file_name();
            let fname = fname.to_string_lossy();
            if !fname.starts_with("temp") || !fname.ends_with("_input") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(t.path()) else {
                continue;
            };
            let Ok(temp_mc) = raw.trim().parse::<i64>() else {
                continue;
            };
            let label_path = path.join(fname.replace("_input", "_label"));
            let sensor = std::fs::read_to_string(&label_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| fname.strip_suffix("_input").unwrap_or(&fname).to_string());
            out.push(HwmonReading {
                name: format!("{chip_name}/{sensor}"),
                temp_mc,
            });
        }
    }
    out
}

fn read_drm_cards() -> Vec<String> {
    #[cfg(unix)]
    {
        let root = Path::new("/sys/class/drm");
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new();
        };
        let mut cards: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("card") && !name.contains('-') {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        cards.sort();
        cards
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

fn read_net_state() -> String {
    #[cfg(unix)]
    {
        read_net_state_linux()
    }
    #[cfg(not(unix))]
    {
        "host".into()
    }
}

#[cfg(unix)]
fn read_net_state_linux() -> String {
    let root = Path::new("/sys/class/net");
    let Ok(entries) = std::fs::read_dir(root) else {
        return "unknown".into();
    };
    for ent in entries.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        if let Ok(op) = std::fs::read_to_string(ent.path().join("operstate")) {
            let op = op.trim().to_string();
            if !op.is_empty() {
                return format!("{name}:{op}");
            }
        }
    }
    "unknown".into()
}

/// journalctl -n 8 (Linux) ou ficheiro `NOSD_JOURNAL_TAIL` / vazio.
fn read_journal_hint() -> JournalHint {
    if let Ok(path) = std::env::var("NOSD_JOURNAL_TAIL") {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let lines: Vec<String> = text.lines().rev().take(8).map(|s| s.to_string()).collect();
            return JournalHint {
                source: format!("file:{path}"),
                lines: lines.into_iter().rev().collect(),
            };
        }
    }
    #[cfg(unix)]
    {
        if let Ok(out) = Command::new("journalctl")
            .args(["-n", "8", "--no-pager", "-o", "cat"])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<String> = text
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                return JournalHint {
                    source: "journalctl".into(),
                    lines,
                };
            }
        }
    }
    JournalHint {
        source: "absent".into(),
        lines: Vec::new(),
    }
}

/// Amostra /sys/class (udev runtime); sem netlink DBus (honesty).
fn read_udev_hint() -> UdevHint {
    #[cfg(unix)]
    {
        let mut devices = Vec::new();
        for class in ["net", "drm", "input"] {
            let root = Path::new("/sys/class").join(class);
            if let Ok(entries) = std::fs::read_dir(&root) {
                for ent in entries.flatten().take(12) {
                    let name = ent.file_name().to_string_lossy().into_owned();
                    devices.push(format!("{class}/{name}"));
                }
            }
        }
        devices.sort();
        devices.truncate(24);
        return UdevHint {
            source: if devices.is_empty() {
                "sysfs_empty".into()
            } else {
                "sysfs".into()
            },
            devices,
        };
    }
    #[cfg(not(unix))]
    {
        UdevHint {
            source: "absent_platform".into(),
            devices: Vec::new(),
        }
    }
}

/// NM: nmcli best-effort ou /sys/class/net (sem DBus linkado).
fn read_nm_hint() -> NmHint {
    #[cfg(unix)]
    {
        if let Ok(out) = Command::new("nmcli")
            .args(["-t", "-f", "STATE", "g"])
            .output()
        {
            if out.status.success() {
                let connectivity = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let primary = Command::new("nmcli")
                    .args(["-t", "-f", "DEVICE,TYPE,STATE", "d"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .find(|l| l.contains(":connected") || l.ends_with(":connected"))
                            .map(|s| s.to_string())
                    });
                return NmHint {
                    source: "nmcli".into(),
                    connectivity,
                    primary,
                };
            }
        }
        return NmHint {
            source: "sysfs_net".into(),
            connectivity: read_net_state_linux(),
            primary: None,
        };
    }
    #[cfg(not(unix))]
    {
        NmHint {
            source: "absent_platform".into(),
            connectivity: "host".into(),
            primary: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_snapshot_smoke() {
        let s = collect_snapshot();
        assert!(!s.journal.source.is_empty());
        assert!(!s.udev.source.is_empty());
        assert!(!s.nm.source.is_empty());
        #[cfg(not(unix))]
        assert!(s.hwmon.is_empty());
    }

    #[test]
    fn thermal_hint_threshold() {
        assert!(!thermal_ngl_reduce(&[]));
        assert!(!thermal_ngl_reduce(&[HwmonReading {
            name: "x".into(),
            temp_mc: 85_000,
        }]));
        assert!(thermal_ngl_reduce(&[HwmonReading {
            name: "x".into(),
            temp_mc: 85_001,
        }]));
    }

    #[test]
    fn presence_env_parse() {
        assert_eq!(Presence::parse_env("drowsy"), Some(Presence::Drowsy));
        assert_eq!(Presence::parse_env("bogus"), None);
        assert!(Presence::Sleep.prefer_vram_release());
    }

    #[test]
    fn ngl_cap_sleep_zero() {
        assert_eq!(suggest_ngl_cap(Presence::Sleep, false), 0);
        assert_eq!(suggest_ngl_cap(Presence::Active, true), 16);
        assert_eq!(suggest_ngl_cap(Presence::Active, false), 99);
    }

    #[test]
    fn bus_payload_roundtrip_fields() {
        let s = collect_snapshot();
        let v = snapshot_bus_payload(&s);
        assert!(v.get("presence").is_some());
        assert!(v.get("suggest_ngl_cap").is_some());
    }
}
