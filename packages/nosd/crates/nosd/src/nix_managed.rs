//! CapGate NIX_WRITE — staging + anel Cortex (auto-apply via serviço root).
//! `nixos-rebuild switch` **nunca** corre como user nosd (ProtectSystem); sinaliza
//! `/var/lib/nosd/nix-apply.request` para `nosd-nix-apply.service` (root).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum NixProposeError {
    #[error("empty nix text")]
    Empty,
    #[error("nix text rejected: {0}")]
    Reject(&'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("apply: {0}")]
    Apply(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct NixProposeResult {
    pub staging_path: String,
    pub bytes: usize,
    pub syntax_ok: bool,
    pub check_tool: String,
    pub hitl_required: bool,
    pub switch_forbidden: bool,
    pub auto_switch_armed: bool,
    pub apply_requested: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NixApplyResult {
    pub requested: bool,
    pub request_path: String,
    pub target: String,
    pub auto_env: bool,
    pub note: String,
}

/// Schema ai-managed (NX.1): deny keys/paths que brickam o host.
const SCHEMA_DENY_SUBSTRINGS: &[&str] = &[
    "boot.loader.efi.canTouchEfiVariables",
    "fileSystems.\"/\"",
    "fileSystems./",
    "disko.",
    "users.users.root.hashedPassword",
    "security.sudo.wheelNeedsPassword = false",
    "networking.hostId",
];

/// Allowlist mínima: parece um módulo Nix, sem `switch` embutido + schema deny-list.
pub fn validate_nix_text(text: &str) -> Result<(), NixProposeError> {
    let t = text.trim();
    if t.is_empty() {
        return Err(NixProposeError::Empty);
    }
    if t.len() > 256 * 1024 {
        return Err(NixProposeError::Reject("too large (>256KiB)"));
    }
    let lower = t.to_ascii_lowercase();
    if lower.contains("nixos-rebuild") && lower.contains("switch") {
        return Err(NixProposeError::Reject("staging must not embed switch"));
    }
    if !(t.contains('{') && t.contains('}')) {
        return Err(NixProposeError::Reject(
            "expected nix attrset/module braces",
        ));
    }
    validate_ai_managed_schema(t)?;
    Ok(())
}

/// NX.1 — deny-list estrutural (não é parser Nix pleno; honesty).
pub fn validate_ai_managed_schema(text: &str) -> Result<(), NixProposeError> {
    for needle in SCHEMA_DENY_SUBSTRINGS {
        if text.contains(needle) {
            return Err(NixProposeError::Reject("schema deny: brick/sensitive key"));
        }
    }
    Ok(())
}

fn hitl_required_env() -> bool {
    match std::env::var("NOSD_NIX_REQUIRE_HITL") {
        Ok(v) => !(v == "0" || v.eq_ignore_ascii_case("false")),
        Err(_) => true,
    }
}

fn run_syntax_check(path: &Path) -> (bool, String) {
    match Command::new("nix-instantiate")
        .args(["--parse"])
        .arg(path)
        .output()
    {
        Ok(out) => (out.status.success(), "nix-instantiate --parse".into()),
        Err(e) => {
            // R-02: ausência da ferramenta é falha, não "structural OK".
            warn!(target: "nosd", "nix-instantiate absent ({e}) - syntax check fail-closed");
            (false, "nix-instantiate absent".into())
        }
    }
}

fn auto_switch_env() -> bool {
    matches!(
        std::env::var("NOSD_NIX_AUTO_SWITCH")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn log_line(msg: &str) {
    let paths = [
        PathBuf::from("/mnt/nixaios-log/nix-apply.log"),
        PathBuf::from("/var/lib/nosd/nix-apply.log"),
    ];
    for p in &paths {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(p) {
            use std::io::Write;
            let _ = writeln!(f, "{msg}");
            break;
        }
    }
}

/// T3.1 (G3): campos tipados de `ai-managed.nix`. O agente propõe valores;
/// o texto Nix nasce de um TEMPLATE fixo (código conhecido), não de string
/// arbitrária — a deny-list continua como segunda camada e o syntax check
/// como terceira. Valores são charset-restrito (sem `"`/`\`/metacaracteres
/// Nix), logo a interpolação no template é segura por construção.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiManagedFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default)]
    pub allow_unfree: bool,
}

/// hostname(1): `[a-zA-Z0-9-]{1,63}`, sem `-` nas pontas.
fn valid_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// locale `lang[_COUNTRY][.codeset]` — só alfanumérico + `._@-`.
fn valid_locale(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
}

/// timezone `Area/City` — alfanumérico + `_/+-`.
fn valid_tz(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '-' | '+'))
}

pub fn validate_ai_managed_fields(f: &AiManagedFields) -> Result<(), NixProposeError> {
    if let Some(h) = &f.hostname {
        if !valid_hostname(h) {
            return Err(NixProposeError::Reject("invalid hostname"));
        }
    }
    if let Some(l) = &f.locale {
        if !valid_locale(l) {
            return Err(NixProposeError::Reject("invalid locale"));
        }
    }
    if let Some(tz) = &f.timezone {
        if !valid_tz(tz) {
            return Err(NixProposeError::Reject("invalid timezone"));
        }
    }
    Ok(())
}

/// Gera o staging a partir de campos tipados (rota de menor risco do ring).
pub fn render_ai_managed(f: &AiManagedFields) -> Result<String, NixProposeError> {
    validate_ai_managed_fields(f)?;
    let mut body = String::from(
        "# ai-managed.nix - GERADO por nosd (T3.1: campos tipados, template fixo).\n\
         # `nix.propose fields={...}` -> staging -> HITL -> helper root (target fixo).\n\
         { config, lib, pkgs, ... }:\n{\n",
    );
    if let Some(h) = &f.hostname {
        body.push_str(&format!("  networking.hostName = \"{h}\";\n"));
    }
    if let Some(l) = &f.locale {
        body.push_str(&format!("  i18n.defaultLocale = \"{l}\";\n"));
    }
    if let Some(tz) = &f.timezone {
        body.push_str(&format!("  time.timeZone = \"{tz}\";\n"));
    }
    body.push_str(&format!(
        "  nixpkgs.config.allowUnfree = {};\n",
        f.allow_unfree
    ));
    body.push_str("}\n");
    Ok(body)
}

/// Escreve staging e valida. Se `NOSD_NIX_AUTO_SWITCH=1`, arma apply root.
pub fn propose_staging(
    data_dir: &Path,
    nix_text: &str,
) -> Result<NixProposeResult, NixProposeError> {
    validate_nix_text(nix_text)?;
    fs::create_dir_all(data_dir)?;
    let staging = data_dir.join("staging-ai-managed.nix");
    if !crate::landlock::path_allowed(&staging, data_dir) {
        return Err(NixProposeError::Reject("staging path outside data_dir"));
    }
    fs::write(&staging, nix_text)?;
    let (syntax_ok, check_tool) = run_syntax_check(&staging);
    let auto = auto_switch_env();
    let hitl = hitl_required_env();
    let mut apply_requested = false;
    let note = if !syntax_ok {
        "syntax failed — apply not armed".into()
    } else if auto && !hitl {
        match request_apply(data_dir) {
            Ok(r) => {
                apply_requested = r.requested;
                r.note
            }
            Err(e) => format!("auto arm failed: {e}"),
        }
    } else if auto && hitl {
        "staging OK — HitlConfirm required before nix.apply (NOSD_NIX_REQUIRE_HITL)".into()
    } else {
        "staging only — HitlConfirm → nix.confirm/apply; build-vm first if nosdNixRing.buildVmFirst"
            .into()
    };
    info!(
        target: "nosd",
        "nix.propose staging={} syntax_ok={} auto={} apply_req={}",
        staging.display(),
        syntax_ok,
        auto,
        apply_requested
    );
    log_line(&format!(
        "propose staging={} syntax_ok={} auto={} apply={}",
        staging.display(),
        syntax_ok,
        auto,
        apply_requested
    ));
    Ok(NixProposeResult {
        staging_path: staging.display().to_string(),
        bytes: nix_text.len(),
        syntax_ok,
        check_tool,
        hitl_required: hitl || !auto,
        switch_forbidden: !auto,
        auto_switch_armed: auto && !hitl,
        apply_requested,
        note,
    })
}

/// NX.1 — pede `nixos-rebuild build-vm` via ficheiro request (root helper / path unit).
pub fn request_build_vm(data_dir: &Path) -> Result<NixApplyResult, NixProposeError> {
    let staging = data_dir.join("staging-ai-managed.nix");
    if !staging.is_file() {
        return Err(NixProposeError::Apply(
            "missing staging-ai-managed.nix".into(),
        ));
    }
    let (syntax_ok, _) = run_syntax_check(&staging);
    if !syntax_ok {
        return Err(NixProposeError::Apply("staging syntax not ok".into()));
    }
    let req_path = data_dir.join("nix-build-vm.request");
    if !crate::landlock::path_allowed(&req_path, data_dir) {
        return Err(NixProposeError::Apply("build-vm path denied".into()));
    }
    let body = format!(
        "staging={}\nts={}\n",
        staging.display(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    fs::write(&req_path, body)?;
    let _ = Command::new("systemctl")
        .args(["start", "nosd-nix-build-vm.service"])
        .output();
    log_line(&format!("build-vm.request → {}", req_path.display()));
    Ok(NixApplyResult {
        requested: true,
        request_path: req_path.display().to_string(),
        target: "build-vm".into(),
        auto_env: false,
        note: "root helper build-vm (se path unit existir); apply aborta se buildVmFirst falhar"
            .into(),
    })
}

/// Sinaliza o serviço root `nosd-nix-apply` (PathExists / poll).
pub fn request_apply(data_dir: &Path) -> Result<NixApplyResult, NixProposeError> {
    let staging = data_dir.join("staging-ai-managed.nix");
    if !staging.is_file() {
        return Err(NixProposeError::Apply(
            "missing staging-ai-managed.nix".into(),
        ));
    }
    let (syntax_ok, _) = run_syntax_check(&staging);
    if !syntax_ok {
        return Err(NixProposeError::Apply("staging syntax not ok".into()));
    }
    let target = std::env::var("NOSD_AI_MANAGED_PATH")
        .unwrap_or_else(|_| "/etc/nixos/ai-managed.nix".into());
    let req_path = data_dir.join("nix-apply.request");
    if !crate::landlock::path_allowed(&req_path, data_dir) {
        return Err(NixProposeError::Apply("apply request path denied".into()));
    }
    let body = format!(
        "staging={}\ntarget={}\nts={}\n",
        staging.display(),
        target,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    fs::write(&req_path, body)?;
    // Kick systemd path/service if available (best-effort)
    let _ = Command::new("systemctl")
        .args(["start", "nosd-nix-apply.service"])
        .output();
    log_line(&format!("apply.request → {}", req_path.display()));
    Ok(NixApplyResult {
        requested: true,
        request_path: req_path.display().to_string(),
        target,
        auto_env: auto_switch_env(),
        note: "root nosd-nix-apply.service will copy staging→ai-managed.nix and switch".into(),
    })
}

/// HITL confirm = mesmo arm que `nix.apply` (SITE_POLICY Confirm/Escalate já gated).
/// G3.2: escreve também `nix-confirm.request` (TTL 600 s no helper root) —
/// com two-phase activado, `nix.apply` só faz copy+build-vm+`test` (boot
/// default intacta); o `switch` exige o segundo pedido (nonce HITL já
/// server-authored no dispatch).
pub fn request_confirm(data_dir: &Path) -> Result<NixApplyResult, NixProposeError> {
    let confirm_path = data_dir.join("nix-confirm.request");
    if !crate::landlock::path_allowed(&confirm_path, data_dir) {
        return Err(NixProposeError::Apply("confirm path denied".into()));
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    fs::write(&confirm_path, format!("ts={ts}\n"))?;
    log_line(&format!("confirm.request → {}", confirm_path.display()));
    let mut r = request_apply(data_dir)?;
    r.note = format!("HITL confirm (G3.2: two-phase; TTL 600s) → {}", r.note);
    Ok(r)
}

/// Rollback: sinaliza `nix-rollback.request` para o helper root (best-effort).
pub fn request_rollback(data_dir: &Path) -> Result<NixApplyResult, NixProposeError> {
    fs::create_dir_all(data_dir)?;
    let req_path = data_dir.join("nix-rollback.request");
    if !crate::landlock::path_allowed(&req_path, data_dir) {
        return Err(NixProposeError::Apply("rollback path denied".into()));
    }
    let generation = std::env::var("NOSD_NIX_ROLLBACK_GEN").unwrap_or_else(|_| "previous".into());
    let body = format!(
        "generation={}\nts={}\n",
        generation,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    fs::write(&req_path, &body)?;
    let _ = Command::new("systemctl")
        .args(["start", "nosd-nix-rollback.service"])
        .output();
    log_line(&format!("rollback.request → {}", req_path.display()));
    Ok(NixApplyResult {
        requested: true,
        request_path: req_path.display().to_string(),
        target: generation,
        auto_env: false,
        note: "Escalate HITL — root nosd-nix-rollback.service (se existir) aplica generation"
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn nix_tool_present() -> bool {
        Command::new("nix-instantiate")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn reject_switch_embed() {
        let t = r#"{ ... }: { system.activationScripts.x = "nixos-rebuild switch"; }"#;
        assert!(validate_nix_text(t).is_err());
    }

    #[test]
    fn propose_writes_staging() {
        let dir = tempdir().unwrap();
        std::env::remove_var("NOSD_NIX_AUTO_SWITCH");
        let r = propose_staging(dir.path(), "{ ... }: { }").unwrap();
        assert!(r.hitl_required);
        assert!(Path::new(&r.staging_path).is_file());
    }

    #[test]
    fn request_rollback_writes() {
        let dir = tempdir().unwrap();
        let r = request_rollback(dir.path()).unwrap();
        assert!(r.requested);
        assert!(Path::new(&r.request_path).is_file());
    }

    #[test]
    fn request_apply_needs_staging() {
        let dir = tempdir().unwrap();
        assert!(request_apply(dir.path()).is_err());
        fs::write(dir.path().join("staging-ai-managed.nix"), "{ ... }: { }").unwrap();
        if nix_tool_present() {
            let r = request_apply(dir.path()).unwrap();
            assert!(r.requested);
        } else {
            // R-02: sem nix-instantiate, falha fechado.
            let e = request_apply(dir.path()).unwrap_err();
            assert!(e.to_string().contains("syntax"), "{e}");
        }
    }

    #[test]
    fn schema_denies_disko() {
        let t = r#"{ ... }: { disko.devices = {}; }"#;
        assert!(validate_nix_text(t).is_err());
    }

    /// T3.1: campos tipados geram o template e recusam injecção.
    #[test]
    fn render_fields_produces_template_and_rejects_injection() {
        let f = AiManagedFields {
            hostname: Some("jarbas-lab".into()),
            locale: Some("pt_PT.UTF-8".into()),
            timezone: Some("Europe/Lisbon".into()),
            allow_unfree: true,
        };
        let t = render_ai_managed(&f).unwrap();
        assert!(t.contains("networking.hostName = \"jarbas-lab\";"));
        assert!(t.contains("i18n.defaultLocale = \"pt_PT.UTF-8\";"));
        assert!(t.contains("time.timeZone = \"Europe/Lisbon\";"));
        assert!(t.contains("allowUnfree = true;"));
        // O texto gerado passa as validações do caminho livre (brace + deny).
        assert!(validate_nix_text(&t).is_ok(), "template deve validar");
        // Metacaracteres Nix/aspas não chegam ao template.
        let evil = AiManagedFields {
            hostname: Some("x\"; boot.loader.efi.canTouchEfiVariables = true;".into()),
            ..Default::default()
        };
        assert!(
            render_ai_managed(&evil).is_err(),
            "hostname com metacaracteres recusado"
        );
        let empty = AiManagedFields {
            hostname: Some(String::new()),
            ..Default::default()
        };
        assert!(render_ai_managed(&empty).is_err());
        let bad_tz = AiManagedFields {
            timezone: Some("Europe/../evil".into()),
            ..Default::default()
        };
        assert!(
            render_ai_managed(&bad_tz).is_err(),
            "'.' fora de charset de tz"
        );
    }

    #[test]
    fn build_vm_request_writes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("staging-ai-managed.nix"), "{ ... }: { }").unwrap();
        if nix_tool_present() {
            let r = request_build_vm(dir.path()).unwrap();
            assert!(r.requested);
            assert!(Path::new(&r.request_path).is_file());
        } else {
            let e = request_build_vm(dir.path()).unwrap_err();
            assert!(e.to_string().contains("syntax"), "{e}");
        }
    }
}
