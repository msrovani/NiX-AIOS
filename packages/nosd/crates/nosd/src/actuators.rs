//! Actuators CapGate-scoped — allowlists + stubs (sem side-effects até HITL).
//!
//! Wi‑Fi: use [`crate::wifi_nm`] (NetworkManager / nmcli) — não duplicar aqui.
//! NX.4: systemctl + PipeWire volume plan + GPU flake hint + DeviceRecipe→Nix staging.

use nos_core::{CapBits, CapGate};
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
}
