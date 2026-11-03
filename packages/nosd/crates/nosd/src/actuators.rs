//! Actuators CapGate-scoped — allowlists + stubs (sem side-effects até HITL).
//!
//! Wi‑Fi: use [`crate::wifi_nm`] (NetworkManager / nmcli) — não duplicar aqui.
//! NX.4: systemctl + PipeWire volume plan + GPU flake hint + DeviceRecipe→Nix staging.

use nos_core::{CapBits, CapGate};
use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;

/// Política: reparticionamento de disco nunca via nosd.
pub const DENY_REPARTITION: &str = "deny_repartition";

/// Política: limite de potência GPU não exposto (vendor-specific / risco).
pub const DENY_GPU_POWER_LIMIT: &str = "deny_gpu_power_limit";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActuatorError {
    #[error("capability: {0}")]
    Cap(&'static str),
    #[error("unit not allowlisted: {0}")]
    UnitDenied(String),
    #[error("action denied by policy: {0}")]
    Policy(&'static str),
    #[error("io: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemctlIntent {
    pub unit: String,
    pub verb: String,
    /// true quando o stub executaria `systemctl` (caller com HITL confirmado).
    pub would_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeWireVolumePlan {
    pub percent: u32,
    pub would_run: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuFlakePlan {
    pub enable_cuda: bool,
    pub drm_hint: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRecipePlan {
    pub recipe_name: String,
    pub nix_fragment: String,
    pub hitl_required: bool,
}

/// Allowlist de unidades systemd tocáveis pelo daemon.
pub fn systemctl_unit_allowed(unit: &str) -> bool {
    let u = unit.trim();
    if u.is_empty() {
        return false;
    }
    u.starts_with("nosd") || u.starts_with("jarbas-") || u.starts_with("nosd-engine@")
}

/// Valida caps + allowlist; devolve intent (stub — não executa sem HITL explícito).
pub fn systemctl_scoped(
    gate: &CapGate,
    cap_gen: u64,
    unit: &str,
    verb: &str,
    hitl_confirmed: bool,
) -> Result<SystemctlIntent, ActuatorError> {
    gate.allow(CapBits::HITL, cap_gen)
        .map_err(ActuatorError::Cap)?;
    if !systemctl_unit_allowed(unit) {
        return Err(ActuatorError::UnitDenied(unit.to_string()));
    }
    let verb = verb.trim();
    if verb.is_empty() {
        return Err(ActuatorError::Io("empty verb".into()));
    }
    Ok(SystemctlIntent {
        unit: unit.to_string(),
        verb: verb.to_string(),
        would_run: hitl_confirmed,
    })
}

/// Stub: executa `systemctl` só se intent.would_run (HITL).
pub fn systemctl_run_stub(intent: &SystemctlIntent) -> Result<(), ActuatorError> {
    if !intent.would_run {
        return Err(ActuatorError::Policy("hitl_required"));
    }
    if !systemctl_unit_allowed(&intent.unit) {
        return Err(ActuatorError::UnitDenied(intent.unit.clone()));
    }
    let out = Command::new("systemctl")
        .arg(&intent.verb)
        .arg(&intent.unit)
        .output()
        .map_err(|e| ActuatorError::Io(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ActuatorError::Io(format!("systemctl exit {}", out.status)))
    }
}

/// PipeWire volume — SITE_POLICY Auto; CapGate OBSERVE para plan, HITL para
/// aplicar. T0.3: a autorização vem da IDENTIDADE (cap HITL do perfil), nunca
/// de um booleano do cliente (`hitl_confirmed` é ignorado — regra 2 do
/// AGENTS.md).
pub fn pipewire_volume_plan(
    gate: &CapGate,
    cap_gen: u64,
    percent: u32,
) -> Result<PipeWireVolumePlan, ActuatorError> {
    gate.allow(CapBits::OBSERVE, cap_gen)
        .map_err(ActuatorError::Cap)?;
    let percent = percent.min(150);
    let would_run = gate.allow(CapBits::HITL, cap_gen).is_ok();
    Ok(PipeWireVolumePlan {
        percent,
        would_run,
        note: if would_run {
            "wpctl set-volume @DEFAULT_AUDIO_SINK@ N% (best-effort)".into()
        } else {
            "plan only — perfil sem HITL; o apply exige grant do servidor".into()
        },
    })
}

/// Dispositivo PipeWire (sink/source) para observe.audio / audio.devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub is_default: bool,
}

/// Vista read-only de volume + inventário wpctl (best-effort).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioView {
    pub volume_percent: Option<u32>,
    pub muted: Option<bool>,
    pub sink: Option<String>,
    pub source: Option<String>,
    pub sinks: Vec<AudioDeviceInfo>,
    pub sources: Vec<AudioDeviceInfo>,
    /// `"wpctl"` quando wpctl respondeu; `"absent"` caso contrário (Windows/sem PipeWire).
    pub source_status: String,
}

fn audio_view_absent() -> AudioView {
    AudioView {
        volume_percent: None,
        muted: None,
        sink: None,
        source: None,
        sinks: vec![],
        sources: vec![],
        source_status: "absent".into(),
    }
}

/// Parse `wpctl get-volume @DEFAULT_AUDIO_SINK@` → (percent 0–150, muted?).
pub fn parse_wpctl_get_volume(stdout: &str) -> (Option<u32>, Option<bool>) {
    let line = stdout.lines().next().unwrap_or("").trim();
    let muted = line.to_ascii_lowercase().contains("muted");
    let vol_part = line
        .split(':')
        .nth(1)
        .unwrap_or(line)
        .split_whitespace()
        .next()
        .unwrap_or("");
    let Ok(v) = vol_part.parse::<f64>() else {
        return (None, muted.then_some(true));
    };
    let pct = if v <= 1.0 + f64::EPSILON {
        (v * 100.0).round() as u32
    } else {
        v.round() as u32
    };
    (Some(pct.min(150)), muted.then_some(true))
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum WpctlSection {
    None,
    Sinks,
    Sources,
}

/// Linha de `wpctl status` (secção Sinks/Sources) → dispositivo.
pub fn parse_wpctl_status_device_line(line: &str, kind: &str) -> Option<AudioDeviceInfo> {
    let raw = line.trim();
    if raw.is_empty() || raw.ends_with(':') {
        return None;
    }
    // Cabeçalhos de árvore (`├─ Sinks:`) — não dispositivos.
    if raw.starts_with('├') || raw.starts_with('└') {
        return None;
    }
    // Prefixos de árvore (`│`, espaços) antes do id / `*`.
    let body = raw
        .trim_start_matches(['│', ' ', '\t'])
        .trim();
    if body.is_empty() {
        return None;
    }
    let is_default = body.starts_with('*') || body.contains(" * ");
    let rest = body.trim_start_matches('*').trim();
    let (id_part, name_part) = rest.split_once('.')?;
    let id = id_part.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let name = name_part
        .split('[')
        .next()
        .unwrap_or(name_part)
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    Some(AudioDeviceInfo {
        id: id.to_string(),
        name,
        kind: kind.to_string(),
        is_default,
    })
}

pub fn parse_wpctl_status(text: &str) -> (Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>) {
    let mut section = WpctlSection::None;
    let mut sinks = Vec::new();
    let mut sources = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Sinks:") || t.contains(" Sinks:") {
            section = WpctlSection::Sinks;
            continue;
        }
        if t.starts_with("Sources:") || t.contains(" Sources:") {
            section = WpctlSection::Sources;
            continue;
        }
        if t.starts_with("Devices:") || t.starts_with("Streams:") {
            section = WpctlSection::None;
            continue;
        }
        match section {
            WpctlSection::Sinks => {
                if let Some(d) = parse_wpctl_status_device_line(line, "sink") {
                    sinks.push(d);
                }
            }
            WpctlSection::Sources => {
                if let Some(d) = parse_wpctl_status_device_line(line, "source") {
                    sources.push(d);
                }
            }
            WpctlSection::None => {}
        }
    }
    (sinks, sources)
}

#[cfg(unix)]
fn wpctl_output(args: &[&str]) -> Option<String> {
    let out = Command::new("wpctl").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Inventário + volume default sink (wpctl); falha silenciosa → `source_status=absent`.
pub fn collect_audio_view() -> AudioView {
    #[cfg(not(unix))]
    {
        return audio_view_absent();
    }
    #[cfg(unix)]
    {
        let vol_text = wpctl_output(&["get-volume", "@DEFAULT_AUDIO_SINK@"]);
        let status_text = wpctl_output(&["status"]);
        if vol_text.is_none() && status_text.is_none() {
            return audio_view_absent();
        }
        let (volume_percent, muted) = vol_text
            .as_deref()
            .map(parse_wpctl_get_volume)
            .unwrap_or((None, None));
        let (sinks, sources) = status_text
            .as_deref()
            .map(parse_wpctl_status)
            .unwrap_or((vec![], vec![]));
        let sink = sinks
            .iter()
            .find(|d| d.is_default)
            .map(|d| d.id.clone())
            .or_else(|| sinks.first().map(|d| d.id.clone()));
        let source = sources
            .iter()
            .find(|d| d.is_default)
            .map(|d| d.id.clone())
            .or_else(|| sources.first().map(|d| d.id.clone()));
        AudioView {
            volume_percent,
            muted,
            sink,
            source,
            sinks,
            sources,
            source_status: "wpctl".into(),
        }
    }
}

/// `wpctl set-default <id>` (sink ou source — o id é o node wpctl).
pub fn pipewire_set_default(kind: &str, id: &str) -> Result<(), ActuatorError> {
    match kind.trim() {
        "sink" | "source" => {}
        _ => return Err(ActuatorError::Io("kind must be sink or source".into())),
    }
    let id = id.trim();
    if id.is_empty() || id.len() > 64 {
        return Err(ActuatorError::Io("invalid device id".into()));
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ActuatorError::Io("device id must be numeric (wpctl)".into()));
    }
    let out = Command::new("wpctl")
        .args(["set-default", id])
        .output()
        .map_err(|e| ActuatorError::Io(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ActuatorError::Io(format!(
            "wpctl set-default exit {} (PipeWire absent?)",
            out.status
        )))
    }
}

pub fn pipewire_mute_apply(muted: bool) -> Result<(), ActuatorError> {
    let arg = if muted { "1" } else { "0" };
    let out = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", arg])
        .output()
        .map_err(|e| ActuatorError::Io(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ActuatorError::Io(format!(
            "wpctl set-mute exit {} (PipeWire absent?)",
            out.status
        )))
    }
}

pub fn pipewire_volume_apply(plan: &PipeWireVolumePlan) -> Result<(), ActuatorError> {
    if !plan.would_run {
        return Err(ActuatorError::Policy("hitl_required"));
    }
    let arg = format!("{}%", plan.percent);
    let out = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &arg])
        .output()
        .map_err(|e| ActuatorError::Io(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ActuatorError::Io(format!(
            "wpctl exit {} (PipeWire absent?)",
            out.status
        )))
    }
}

/// GPU flake toggle — plan only; power-limit Deny (SITE_POLICY).
pub fn gpu_flake_plan(enable_cuda: bool, drm_card: &str) -> Result<GpuFlakePlan, ActuatorError> {
    let drm = if drm_card.trim().is_empty() {
        "/dev/dri/card1".into()
    } else {
        drm_card.trim().to_string()
    };
    Ok(GpuFlakePlan {
        enable_cuda,
        drm_hint: drm,
        note: if enable_cuda {
            "propose services.nosdEngine.enable=true + llama-cpp-cuda (HITL nix.propose)".into()
        } else {
            "propose ngl=0 / CPU llama (HITL nix.propose)".into()
        },
    })
}

/// DeviceRecipe → fragmento Nix staging (não switch).
pub fn device_recipe_to_nix(recipe_name: &str) -> Result<DeviceRecipePlan, ActuatorError> {
    let name = recipe_name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ActuatorError::Io("invalid recipe name".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ActuatorError::Io("recipe charset".into()));
    }
    let nix_fragment = format!(
        r#"{{ config, lib, ... }}: {{
  # DeviceRecipe/{name} — NX.4 thin; firmware blobs = runtime HITL (S1.2b)
  networking.networkmanager.enable = lib.mkDefault true;
  # TODO: import recipe overlays when PackageHub ships RECIPE.md
}}"#
    );
    Ok(DeviceRecipePlan {
        recipe_name: name.into(),
        nix_fragment,
        hitl_required: true,
    })
}

pub fn deny_repartition_reason() -> &'static str {
    DENY_REPARTITION
}

pub fn deny_gpu_power_limit_reason() -> &'static str {
    DENY_GPU_POWER_LIMIT
}

/// DPMS / blank do ecrã (tier idle 2). Best-effort `xset`; senão honesty absent.
pub fn display_blank(blank: bool) -> Result<String, ActuatorError> {
    #[cfg(unix)]
    {
        let verb = if blank { "off" } else { "on" };
        if std::env::var_os("DISPLAY").is_some() {
            let out = Command::new("xset")
                .args(["dpms", "force", verb])
                .output()
                .map_err(|e| ActuatorError::Io(e.to_string()))?;
            if out.status.success() {
                return Ok(format!("xset_dpms_{verb}"));
            }
        }
        Ok("display_blank_absent".into())
    }
    #[cfg(not(unix))]
    {
        let _ = blank;
        Ok("display_blank_absent_platform".into())
    }
}

/// Suspend: só plan/HITL — nunca auto.
pub fn system_suspend_plan(hitl_ok: bool) -> Result<String, ActuatorError> {
    if !hitl_ok {
        return Err(ActuatorError::Policy("hitl_required"));
    }
    #[cfg(unix)]
    {
        Ok("systemctl suspend (grant system.suspend; metal AWAITING)".into())
    }
    #[cfg(not(unix))]
    {
        Ok("suspend_absent_platform".into())
    }
}

pub fn system_suspend_apply(hitl_ok: bool) -> Result<(), ActuatorError> {
    if !hitl_ok {
        return Err(ActuatorError::Policy("hitl_required"));
    }
    #[cfg(unix)]
    {
        let out = Command::new("systemctl")
            .args(["suspend"])
            .output()
            .map_err(|e| ActuatorError::Io(e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(ActuatorError::Io(format!(
                "systemctl suspend exit {}",
                out.status
            )))
        }
    }
    #[cfg(not(unix))]
    {
        Err(ActuatorError::Io("suspend_absent_platform".into()))
    }
}

/// Hibernate capability probe (honesty). Sem `NOSD_HIBERNATE=1` → disabled.
pub fn hibernate_capability() -> (bool, String) {
    if std::env::var("NOSD_HIBERNATE").ok().as_deref() != Some("1") {
        return (false, "NOSD_HIBERNATE!=1".into());
    }
    #[cfg(unix)]
    {
        if let Ok(s) = std::fs::read_to_string("/sys/power/disk") {
            let t = s.trim();
            if t.contains("[platform]") || t.contains("[shutdown]") || t.contains("platform") {
                return (true, format!("sysfs_disk={t}"));
            }
            return (false, format!("sysfs_disk={t}"));
        }
        (false, "sysfs_power_disk_absent".into())
    }
    #[cfg(not(unix))]
    {
        (false, "hibernate_absent_platform".into())
    }
}

pub fn system_hibernate_plan(hitl_ok: bool) -> Result<String, ActuatorError> {
    if !hitl_ok {
        return Err(ActuatorError::Policy("hitl_required"));
    }
    let (ok, note) = hibernate_capability();
    if !ok {
        return Err(ActuatorError::Io(format!("hibernate_absent:{note}")));
    }
    Ok(format!(
        "systemctl hibernate (grant system.hibernate; {note})"
    ))
}

pub fn system_hibernate_apply(hitl_ok: bool) -> Result<(), ActuatorError> {
    if !hitl_ok {
        return Err(ActuatorError::Policy("hitl_required"));
    }
    let (ok, note) = hibernate_capability();
    if !ok {
        return Err(ActuatorError::Io(format!("hibernate_absent:{note}")));
    }
    #[cfg(unix)]
    {
        let out = Command::new("systemctl")
            .args(["hibernate"])
            .output()
            .map_err(|e| ActuatorError::Io(e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(ActuatorError::Io(format!(
                "systemctl hibernate exit {}",
                out.status
            )))
        }
    }
    #[cfg(not(unix))]
    {
        Err(ActuatorError::Io("hibernate_absent_platform".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_units() {
        assert!(systemctl_unit_allowed("nosd.service"));
        assert!(systemctl_unit_allowed("jarbas-cage.service"));
        assert!(systemctl_unit_allowed("nosd-engine@llama.service"));
        assert!(!systemctl_unit_allowed("sshd.service"));
    }

    #[test]
    fn scoped_requires_hitl_cap() {
        let gate = CapGate::new(CapBits::OBSERVE, 1);
        assert!(systemctl_scoped(&gate, 1, "nosd.service", "restart", true).is_err());
    }

    #[test]
    fn policy_constants() {
        assert_eq!(deny_repartition_reason(), DENY_REPARTITION);
        assert_eq!(deny_gpu_power_limit_reason(), DENY_GPU_POWER_LIMIT);
    }

    #[test]
    fn pipewire_plan_ok() {
        // T0.3: sem booleano do cliente — a cap HITL da identidade decide.
        let gate = CapGate::new(CapBits::OBSERVE, 1);
        let p = pipewire_volume_plan(&gate, 1, 80).unwrap();
        assert_eq!(p.percent, 80);
        assert!(!p.would_run, "LocalUser (só OBSERVE) leva plan only");
        let gate_admin = CapGate::new(CapBits::OBSERVE.union(CapBits::HITL), 1);
        let p2 = pipewire_volume_plan(&gate_admin, 1, 80).unwrap();
        assert!(p2.would_run, "perfil com HITL aplica");
    }

    #[test]
    fn gpu_plan_cuda() {
        let p = gpu_flake_plan(true, "").unwrap();
        assert!(p.enable_cuda);
        assert!(p.note.contains("nosdEngine"));
    }

    #[test]
    fn recipe_nix_fragment() {
        let r = device_recipe_to_nix("ath10k-qca").unwrap();
        assert!(r.nix_fragment.contains("ath10k-qca"));
        assert!(r.hitl_required);
        assert!(device_recipe_to_nix("../evil").is_err());
    }

    #[test]
    fn collect_audio_view_does_not_panic() {
        let _ = collect_audio_view();
    }

    #[test]
    fn parse_wpctl_get_volume_smoke() {
        let (pct, muted) = parse_wpctl_get_volume("Volume: 0.650000\n");
        assert_eq!(pct, Some(65));
        assert_eq!(muted, None);
        let (pct2, muted2) = parse_wpctl_get_volume("Volume: 0.40 [MUTED]\n");
        assert_eq!(pct2, Some(40));
        assert_eq!(muted2, Some(true));
    }

    #[test]
    fn parse_wpctl_status_smoke() {
        let sample = r"
Audio
 ├─ Sinks:
 │      39. Dummy Output                        [vol: 0.40]
 │  *   55. Built-in Audio Analog Stereo        [vol: 0.65]
 ├─ Sources:
 │  *   56. Built-in Audio Analog Stereo        [vol: 1.00]
";
        let (sinks, sources) = parse_wpctl_status(sample);
        assert_eq!(sinks.len(), 2);
        assert!(sinks.iter().any(|d| d.id == "55" && d.is_default));
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, "56");
    }
}
