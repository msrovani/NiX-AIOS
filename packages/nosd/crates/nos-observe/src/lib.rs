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

/// Estado de associação para o HUD (ORB): `UP` verde só com rede associada.
///
/// - `associated`: interface up **e** com SSID/carrier real (wifi com SSID,
///   ou Ethernet com `operstate=up` + link negociado). Só aqui o chip fica
///   verde e `netUp` diz "UP".
/// - `no-link`: interface up administrativo mas **sem** associação (Wi-Fi
///   não associado = `nmcli` sem SSID). Chip âmbar `SEM LINK`.
/// - `down`: interface/operstate down (ou plataforma sem leitura).
///
/// Derivação pura (sem I/O): testável por `net_link_state()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetLinkState {
    Associated,
    NoLink,
    Down,
}

impl NetLinkState {
    pub fn as_str(self) -> &'static str {
        match self {
            NetLinkState::Associated => "associated",
            NetLinkState::NoLink => "no-link",
            NetLinkState::Down => "down",
        }
    }
}

/// Deriva `NetLinkState` de (up, operstate, ssid, link_mbps):
/// `UP` verde exige SSID ou Ethernet negociada; interface up sem
/// associação = `no-link` (âmbar), nunca verde.
///
/// - wifi: `associated` iff `up && ssid.is_some()`.
/// - ethernet: `associated` iff `up && operstate == Some("up") &&
///   link_mbps.is_some()`.
/// - outros tipos: `associated` iff `up && operstate == Some("up")`.
/// - `up == false` (ou operstate down) → `down`.
pub fn net_link_state(
    up: bool,
    net_type: &str,
    operstate: Option<&str>,
    ssid: Option<&str>,
    link_mbps: Option<i64>,
) -> NetLinkState {
    if !up {
        return NetLinkState::Down;
    }
    if net_type == "wifi" {
        return match ssid {
            Some(s) if !s.trim().is_empty() => NetLinkState::Associated,
            _ => NetLinkState::NoLink,
        };
    }
    if net_type == "ethernet" {
        let carrier = operstate == Some("up") && link_mbps.is_some();
        return if carrier {
            NetLinkState::Associated
        } else {
            NetLinkState::NoLink
        };
    }
    // Demais tipos: só `up` sem operstate "down".
    if operstate == Some("down") || operstate == Some("unknown") {
        return NetLinkState::Down;
    }
    NetLinkState::Associated
}

/// Vista read-only da ligação primária (ORB Fase 1). Sem PSK/secrets; sem throughput inventado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetView {
    /// Legado: `up administrativo`. **Não** é o sinal de "rede utilizável";
    /// usar `link_state == Associated` para verde. Mantido por contrato.
    pub up: bool,
    /// Estado derivado do link real (`associated` | `no-link` | `down`).
    pub link_state: NetLinkState,
    #[serde(rename = "type")]
    pub net_type: String,
    pub device: Option<String>,
    pub link_mbps: Option<i64>,
    pub ssid: Option<String>,
    pub operstate: Option<String>,
    pub source: String,
}

pub fn collect_net_view() -> NetView {
    #[cfg(unix)]
    {
        collect_net_view_unix()
    }
    #[cfg(not(unix))]
    {
        NetView {
            up: false,
            link_state: NetLinkState::Down,
            net_type: "none".into(),
            device: None,
            link_mbps: None,
            ssid: None,
            operstate: None,
            source: "absent_platform".into(),
        }
    }
}

/// Linha `nmcli -t -f DEVICE,TYPE,STATE d` → (device, type, state).
pub fn parse_nmcli_device_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.splitn(3, ':');
    let device = parts.next()?.trim().to_string();
    let net_type = parts.next()?.trim().to_string();
    let state = parts.next()?.trim().to_string();
    if device.is_empty() {
        return None;
    }
    Some((device, net_type, state))
}

/// Primeira linha NM com estado connected (DEVICE, TYPE).
pub fn pick_nmcli_connected_device(lines: &str) -> Option<(String, String)> {
    for line in lines.lines() {
        let Some((dev, ty, state)) = parse_nmcli_device_line(line) else {
            continue;
        };
        if state == "connected" || state.ends_with(":connected") || state.contains(":connected") {
            return Some((dev, ty));
        }
    }
    None
}

/// `/sys/class/net/<dev>/speed`: valores negativos (ex.: -1) → None.
pub fn parse_sysfs_link_mbps(raw: &str) -> Option<i64> {
    let v = raw.trim().parse::<i64>().ok()?;
    (v >= 0).then_some(v)
}

/// TYPE do nmcli → wifi|ethernet|other.
pub fn normalize_nmcli_type(ty: &str) -> String {
    match ty.trim().to_ascii_lowercase().as_str() {
        "wifi" | "802-11-wireless" => "wifi".into(),
        "ethernet" | "802-3-ethernet" | "wired" => "ethernet".into(),
        _ => "other".into(),
    }
}

/// Linha `nmcli -t -f ACTIVE,SSID d wifi` com ACTIVE=yes.
pub fn parse_active_wifi_ssid_line(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.splitn(2, ':');
    let active = parts.next()?.trim();
    let ssid = parts.next()?.trim();
    if !active.eq_ignore_ascii_case("yes") || ssid.is_empty() {
        return None;
    }
    Some(ssid.to_string())
}

#[cfg(unix)]
fn collect_net_view_unix() -> NetView {
    let mut source = "sysfs".to_string();
    let nmcli_dev = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "d"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());

    let (device, type_hint) = if let Some(ref text) = nmcli_dev {
        if let Some((dev, ty)) = pick_nmcli_connected_device(text) {
            source = "nmcli".into();
            (Some(dev), Some(ty))
        } else {
            (pick_first_operstate_up_device(), None)
        }
    } else {
        (pick_first_operstate_up_device(), None)
    };

    let Some(dev) = device else {
        return NetView {
            up: false,
            link_state: NetLinkState::Down,
            net_type: "none".into(),
            device: None,
            link_mbps: None,
            ssid: None,
            operstate: None,
            source,
        };
    };

    let operstate = read_iface_operstate(&dev);
    let up = operstate.as_deref() == Some("up");
    let link_mbps = read_iface_link_mbps(&dev);
    let net_type = type_hint
        .as_deref()
        .map(normalize_nmcli_type)
        .unwrap_or_else(|| detect_net_type_sysfs(&dev));
    let ssid = if net_type == "wifi" {
        read_active_wifi_ssid()
    } else {
        None
    };
    let link_state = net_link_state(
        up,
        &net_type,
        operstate.as_deref(),
        ssid.as_deref(),
        link_mbps,
    );

    NetView {
        up,
        link_state,
        net_type,
        device: Some(dev),
        link_mbps,
        ssid,
        operstate,
        source,
    }
}

#[cfg(unix)]
fn pick_first_operstate_up_device() -> Option<String> {
    let root = Path::new("/sys/class/net");
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    for ent in entries.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        if read_iface_operstate(&name).as_deref() == Some("up") {
            return Some(name);
        }
    }
    None
}

#[cfg(unix)]
fn read_iface_operstate(dev: &str) -> Option<String> {
    let path = Path::new("/sys/class/net").join(dev).join("operstate");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(unix)]
fn read_iface_link_mbps(dev: &str) -> Option<i64> {
    let path = Path::new("/sys/class/net").join(dev).join("speed");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| parse_sysfs_link_mbps(&s))
}

#[cfg(unix)]
fn detect_net_type_sysfs(dev: &str) -> String {
    let base = Path::new("/sys/class/net").join(dev);
    if base.join("wireless").exists() {
        return "wifi".into();
    }
    if let Ok(raw) = std::fs::read_to_string(base.join("type")) {
        if let Ok(t) = raw.trim().parse::<u32>() {
            return match t {
                1 => "ethernet".into(),
                772 => "wifi".into(),
                _ => "other".into(),
            };
        }
    }
    "other".into()
}

#[cfg(unix)]
fn read_active_wifi_ssid() -> Option<String> {
    let out = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID", "d", "wifi"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(ssid) = parse_active_wifi_ssid_line(line) {
            return Some(ssid);
        }
    }
    None
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
        UdevHint {
            source: if devices.is_empty() {
                "sysfs_empty".into()
            } else {
                "sysfs".into()
            },
            devices,
        }
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
        NmHint {
            source: "sysfs_net".into(),
            connectivity: read_net_state_linux(),
            primary: None,
        }
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

    #[test]
    fn collect_net_view_smoke() {
        let v = collect_net_view();
        assert!(!v.source.is_empty());
        #[cfg(not(unix))]
        {
            assert!(!v.up);
            assert_eq!(v.net_type, "none");
            assert_eq!(v.source, "absent_platform");
        }
    }

    #[test]
    fn parse_nmcli_device_line_ok() {
        let (d, t, s) = parse_nmcli_device_line("wlan0:wifi:connected").unwrap();
        assert_eq!(d, "wlan0");
        assert_eq!(t, "wifi");
        assert_eq!(s, "connected");
    }

    #[test]
    fn pick_nmcli_connected_skips_disconnected() {
        let text = "eth0:ethernet:disconnected\nwlan0:wifi:connected\n";
        let (d, t) = pick_nmcli_connected_device(text).unwrap();
        assert_eq!(d, "wlan0");
        assert_eq!(t, "wifi");
    }

    #[test]
    fn parse_sysfs_link_mbps_honest() {
        assert_eq!(parse_sysfs_link_mbps("1000"), Some(1000));
        assert_eq!(parse_sysfs_link_mbps("-1"), None);
        assert_eq!(parse_sysfs_link_mbps("bogus"), None);
    }

    #[test]
    fn normalize_nmcli_type_maps() {
        assert_eq!(normalize_nmcli_type("802-11-wireless"), "wifi");
        assert_eq!(normalize_nmcli_type("802-3-ethernet"), "ethernet");
        assert_eq!(normalize_nmcli_type("bridge"), "other");
    }

    #[test]
    fn parse_active_wifi_ssid_line_active_only() {
        assert_eq!(
            parse_active_wifi_ssid_line("yes:MyNet"),
            Some("MyNet".into())
        );
        assert_eq!(parse_active_wifi_ssid_line("no:Hidden"), None);
        assert_eq!(parse_active_wifi_ssid_line("yes:"), None);
    }

    #[test]
    fn net_link_state_derives_from_link_not_iff_up() {
        use NetLinkState as S;
        // Wi-Fi sem SSID = nunca verde, mesmo com iff up.
        assert_eq!(
            net_link_state(true, "wifi", Some("up"), None, Some(150)),
            S::NoLink
        );
        assert_eq!(
            net_link_state(true, "wifi", Some("up"), Some(""), Some(150)),
            S::NoLink
        );
        // Wi-Fi com SSID = associado.
        assert_eq!(
            net_link_state(true, "wifi", Some("up"), Some("Casa"), Some(150)),
            S::Associated
        );
        // Ethernet: exige link negociado (speed presente) + operstate up.
        assert_eq!(
            net_link_state(true, "ethernet", Some("up"), None, Some(1000)),
            S::Associated
        );
        assert_eq!(
            net_link_state(true, "ethernet", Some("up"), None, None),
            S::NoLink
        );
        // Interface down = down (precede tudo).
        assert_eq!(
            net_link_state(false, "wifi", Some("down"), Some("Casa"), Some(150)),
            S::Down
        );
        assert_eq!(
            net_link_state(true, "other", Some("down"), None, None),
            S::Down
        );
        // Strings estáveis para o HUd.
        assert_eq!(S::NoLink.as_str(), "no-link");
        assert_eq!(S::Associated.as_str(), "associated");
        assert_eq!(S::Down.as_str(), "down");
    }
}
