//! Transmutação NixOS Gráfico → Neural AIOS Full (Hotswap + Persistência + Voz/MoE Local).
//! Lê o idioma/fuso/Wi‑Fi configurados na GUI inicial, baixa modelos HF (MoE GGUF + Whisper + Piper)
//! na partição persistente RW e executa o Hotswap desligando o Desktop antigo e subindo o JARBAS.

use nos_core::ModelSlot;
use nos_cortex::{
    fetch_to_dir, scan_path, slot_from_path, suggest_for_capacity, CapacityProber,
    HotSwapController, ModelHub, NodeCapacity, QuantSuggest,
};
use nos_voice::{LocaleVoiceSpec, VoicePipeline, VoiceStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoeDownloadSpec {
    pub slot: String,
    pub repo: String,
    pub file: String,
    pub local_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmuteStatus {
    pub mode: String,
    pub locale: LocaleVoiceSpec,
    pub timezone: String,
    pub wifi_ssid: Option<String>,
    pub persist_mounted: bool,
    pub persist_dir: String,
    pub models_dir: String,
    pub capacity: NodeCapacity,
    pub quant_suggest: QuantSuggest,
    pub recommended_moe: Vec<MoeDownloadSpec>,
    pub voice: VoiceStatus,
    pub registered_slots: usize,
    pub active_engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmuteRunOptions {
    #[serde(default = "default_true")]
    pub download_models: bool,
    #[serde(default = "default_true")]
    pub speak_greeting: bool,
    #[serde(default = "default_true")]
    pub hotswap_now: bool,
    pub locale_override: Option<String>,
    /// JARBAS Confirm — obrigatório para downloads externos (runtime-only policy).
    #[serde(default)]
    pub hitl_confirmed: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TransmuteRunOptions {
    fn default() -> Self {
        Self {
            download_models: true,
            speak_greeting: true,
            hotswap_now: true,
            locale_override: None,
            hitl_confirmed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmuteRunResult {
    pub ok: bool,
    pub locale: String,
    pub models_dir: String,
    pub downloaded: Vec<String>,
    pub errors: Vec<String>,
    pub tts_greeting_played: bool,
    pub hotswap_triggered: bool,
    pub note: String,
}

/// 0.5.0 Linux-first: modelos em `data_dir/models` por omissão.
/// FAT `/mnt/nixaios-log` só com `NIXAIOS_USE_FAT_PERSIST=1` + mount + dirs graváveis pelo processo.
pub fn resolve_models_dir(data_dir: &Path) -> (PathBuf, bool, String) {
    let local_models = data_dir.join("models");
    let _ = fs::create_dir_all(&local_models);

    let fat_opt_in = std::env::var("NIXAIOS_USE_FAT_PERSIST")
        .ok()
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !fat_opt_in {
        return (local_models, false, data_dir.display().to_string());
    }

    let fat_mnt = Path::new("/mnt/nixaios-log");
    let persist_root = fat_mnt.join("nosd-persist");
    let mounted = fat_mnt.is_dir()
        && Command::new("mountpoint")
            .arg("-q")
            .arg(fat_mnt)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

    if mounted {
        let p_models = persist_root.join("models");
        // Só usar FAT se o processo actual conseguir criar/escrever (evita EACCES pós-Q5).
        if fs::create_dir_all(&p_models).is_ok() {
            let probe = p_models.join(".nosd-write-probe");
            if fs::write(&probe, b"ok").is_ok() {
                let _ = fs::remove_file(&probe);
                return (p_models, true, persist_root.display().to_string());
            }
        }
        warn!(
            target: "nosd",
            "NIXAIOS_USE_FAT_PERSIST=1 mas FAT nao gravavel — fallback {}",
            local_models.display()
        );
    }
    (local_models, false, data_dir.display().to_string())
}

pub fn recommended_moe_for(cap: &NodeCapacity) -> Vec<MoeDownloadSpec> {
    let vram = cap.vram_free_mb;
    let ram = cap.ram_avail_mb;
    let use_3b = vram >= 5_500 || ram >= 12_000;

    let (s1_repo, s1_file, s1_local) = if use_3b {
        (
            "Qwen/Qwen2.5-3B-Instruct-GGUF",
            "qwen2.5-3b-instruct-q4_k_m.gguf",
            "active-qwen2.5-3b-q4_k_m.gguf",
        )
    } else {
        (
            "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
            "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            "active-qwen2.5-1.5b-q4_k_m.gguf",
        )
    };

    vec![
        MoeDownloadSpec {
            slot: "active".into(),
            repo: s1_repo.into(),
            file: s1_file.into(),
            local_filename: s1_local.into(),
        },
        MoeDownloadSpec {
            slot: "rust_coder".into(),
            repo: "Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF".into(),
            file: "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf".into(),
            local_filename: "coder-qwen2.5-1.5b-q4_k_m.gguf".into(),
        },
    ]
}

fn detect_timezone() -> String {
    if let Ok(tz) = fs::read_to_string("/etc/timezone") {
        let t = tz.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(link) = fs::read_link("/etc/localtime") {
        let s = link.display().to_string();
        if let Some(idx) = s.find("zoneinfo/") {
            return s[idx + 9..].to_string();
        }
    }
    "America/Sao_Paulo".into()
}

fn detect_active_wifi() -> Option<String> {
    let out = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID", "dev", "wifi"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(ssid) = line
            .strip_prefix("yes:")
            .or_else(|| line.strip_prefix("sim:"))
        {
            let s = ssid.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn read_mode(data_dir: &Path) -> String {
    // Linux-first; FAT só se opt-in (espelha resolve_models_dir).
    let mut paths = vec![data_dir.join("aios-mode")];
    let fat_opt_in = std::env::var("NIXAIOS_USE_FAT_PERSIST")
        .ok()
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if fat_opt_in {
        paths.push(PathBuf::from("/mnt/nixaios-log/nosd-persist/aios-mode"));
    }
    for p in paths {
        if let Ok(s) = fs::read_to_string(p) {
            let t = s.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    "graphical_bootstrap".into()
}

pub fn query_status(
    data_dir: &Path,
    hub: &ModelHub,
    hotswap: &HotSwapController,
) -> TransmuteStatus {
    let (models_dir, persist_mounted, persist_dir) = resolve_models_dir(data_dir);
    let cap = CapacityProber::probe();
    let quant_suggest = suggest_for_capacity(&cap);
    let recommended_moe = recommended_moe_for(&cap);
    let voice_pipe = VoicePipeline::with_models_dir(models_dir.clone());
    let voice = voice_pipe.status();

    TransmuteStatus {
        mode: read_mode(data_dir),
        locale: voice.locale.clone(),
        timezone: detect_timezone(),
        wifi_ssid: detect_active_wifi(),
        persist_mounted,
        persist_dir,
        models_dir: models_dir.display().to_string(),
        capacity: cap,
        quant_suggest,
        recommended_moe,
        voice,
        registered_slots: hub.list().len(),
        active_engine: hotswap.engine_name().to_string(),
    }
}

pub fn execute_transmute(
    data_dir: &Path,
    hub: &ModelHub,
    hotswap: &HotSwapController,
    opts: TransmuteRunOptions,
) -> TransmuteRunResult {
    let (models_dir, persist_mounted, persist_root) = resolve_models_dir(data_dir);
    let mut downloaded = Vec::new();
    let mut errors = Vec::new();

    // 1. Fixar idioma escolhido pelo usuário
    let loc = match opts.locale_override.as_deref() {
        Some(s) if !s.trim().is_empty() => LocaleVoiceSpec::from_locale_str(s),
        _ => LocaleVoiceSpec::detect(),
    };
    std::env::set_var("NOSD_LOCALE", &loc.raw_locale);
    let _ = fs::create_dir_all(data_dir);
    let _ = fs::write(
        data_dir.join("locale.conf"),
        format!("LANG={}\n", loc.raw_locale),
    );
    if persist_mounted {
        let _ = fs::write(
            Path::new(&persist_root).join("locale.conf"),
            format!("LANG={}\n", loc.raw_locale),
        );
    }

    // 2. Configurar dGPU offload se houver GPU dedicada / VRAM
    let cap = CapacityProber::probe();
    let suggest = suggest_for_capacity(&cap);
    if suggest.suggest_ngl > 0 {
        std::env::set_var("NOSD_LLAMA_NGL", suggest.suggest_ngl.to_string());
    }

    // 3. Baixar modelos (Whisper STT + Piper TTS no idioma + MoE GGUF)
    // Runtime-only: Cortex→Hermes deliberam; JARBAS hitl_confirmed obrigatório.
    if opts.download_models {
        let blob_need = nos_hermes::BlobNeed {
            kind: nos_hermes::BlobKind::ModelGguf,
            goal: format!("transmute locale={}", loc.raw_locale),
            source: "transmute.bundle".into(),
            estimated_mb: 1200,
        };
        let plan = nos_hermes::deliberate_blob_fetch(
            &blob_need,
            &cap,
            opts.hitl_confirmed,
            nos_core::s1_scorer_absent(),
        );
        if !plan.allow_fetch {
            errors.push(format!(
                "blob_deliberate: {} (iso: {})",
                plan.reason,
                nos_hermes::iso_must_not_ship_blobs()
            ));
            return TransmuteRunResult {
                ok: false,
                locale: loc.raw_locale.clone(),
                models_dir: models_dir.display().to_string(),
                downloaded,
                errors,
                tts_greeting_played: false,
                hotswap_triggered: false,
                note: format!(
                    "await JARBAS HITL for runtime blob fetch; disposition={:?}",
                    plan.disposition
                ),
            };
        }
        info!(
            target: "nosd",
            "transmute downloading models into {} (locale={} deliberation=ok)",
            models_dir.display(),
            loc.raw_locale
        );
        match fetch_to_dir(&loc.whisper_model_url, &models_dir, &loc.whisper_model_file) {
            Ok(p) => downloaded.push(p.display().to_string()),
            Err(e) => errors.push(format!("whisper: {e}")),
        }
        let piper_onnx_name = format!("{}.onnx", loc.piper_voice);
        let piper_json_name = format!("{}.onnx.json", loc.piper_voice);
        match fetch_to_dir(&loc.piper_onnx_url, &models_dir, &piper_onnx_name) {
            Ok(p) => downloaded.push(p.display().to_string()),
            Err(e) => errors.push(format!("piper onnx: {e}")),
        }
        match fetch_to_dir(&loc.piper_json_url, &models_dir, &piper_json_name) {
            Ok(p) => downloaded.push(p.display().to_string()),
            Err(e) => errors.push(format!("piper json: {e}")),
        }
        // Se o binário piper não estiver no PATH, baixa a release standalone x86_64
        if nos_voice::find_piper_cli().is_none() {
            let tar_url = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz";
            if let Ok(tar_path) = fetch_to_dir(tar_url, &models_dir, "piper_linux_x86_64.tar.gz") {
                let _ = Command::new("tar")
                    .args([
                        "-xzf",
                        &tar_path.display().to_string(),
                        "-C",
                        &models_dir.display().to_string(),
                    ])
                    .status();
                let bin = models_dir.join("piper").join("piper");
                if bin.is_file() {
                    std::env::set_var("NOSD_PIPER_CLI", bin.display().to_string());
                    downloaded.push(bin.display().to_string());
                }
            }
        }

        for spec in recommended_moe_for(&cap) {
            let url = nos_cortex::hf_resolve_url(&spec.repo, &spec.file);
            match fetch_to_dir(&url, &models_dir, &spec.local_filename) {
                Ok(p) => downloaded.push(p.display().to_string()),
                Err(e) => errors.push(format!("moe {}: {e}", spec.slot)),
            }
        }
    }

    // 4. Registar todos os GGUF no ModelHub + anexar Active na dGPU
    for meta in scan_path(&models_dir, 64) {
        if meta.kind != "gguf" {
            continue;
        }
        let len = fs::metadata(&meta.path).map(|m| m.len()).unwrap_or(0);
        let slot = slot_from_path(&meta.path);
        hub.register_path(slot, meta.path.clone(), len);
    }
    if let Some(active_path) = hub.path_of(ModelSlot::Active) {
        if let Err(e) = hotswap.hot_load_llama(active_path) {
            warn!(target: "nosd", "transmute hot_load_llama warn: {e}");
        }
    }

    // 5. Smoke test de voz (TTS saudação no idioma selecionado)
    let voice_pipe = VoicePipeline::with_models_dir(models_dir.clone());
    let tts_greeting_played = if opts.speak_greeting {
        voice_pipe.synthesize_wav(&loc.greeting, true).is_ok()
    } else {
        false
    };

    // 6. Gravar estado de transmutação + disparar Hotswap (desliga Desktop antigo → sobe JARBAS)
    let _ = fs::write(data_dir.join("aios-mode"), "aios\n");
    if persist_mounted {
        let _ = fs::write(Path::new(&persist_root).join("aios-mode"), "aios\n");
    }

    let mut hotswap_triggered = false;
    if opts.hotswap_now {
        let req_file = data_dir.join("hotswap-aios.request");
        if let Err(e) = fs::write(
            &req_file,
            format!(
                "locale={}\nmodels_dir={}\nngl={}\npersist_mounted={}\n",
                loc.raw_locale,
                models_dir.display(),
                suggest.suggest_ngl,
                persist_mounted
            ),
        ) {
            errors.push(format!("hotswap request write: {e}"));
        } else {
            // Mecanismo primário: o ficheiro pedido + `nosd-hotswap-aios.path`
            // (systemd PathExists) dispara o unit. NÃO é preciso root.
            // `systemctl start` é best-effort: nosd corre como user `nosd` e
            // sem polkit devolve "interactive authentication required" (1);
            // isso não é falha do hotswap.
            hotswap_triggered = true;
            match Command::new("systemctl")
                .args(["start", "--no-block", "nosd-hotswap-aios.service"])
                .status()
            {
                Ok(st) if st.success() => {}
                Ok(st) => warn!(
                    target: "nosd",
                    "systemctl hotswap {st} (sem root/polkit); .path trata do pedido"
                ),
                Err(e) => warn!(target: "nosd", "systemctl hotswap indisponível: {e}; .path trata"),
            }
        }
    }

    let note = if !errors.is_empty() {
        format!(
            "Transmute parcial ({} erro(s)); hotswap_triggered={hotswap_triggered}; models={}.",
            errors.len(),
            models_dir.display()
        )
    } else if hotswap_triggered {
        format!(
            "Downloads OK; hotswap disparado (log /var/lib/nosd/hotswap.log); models={}.",
            models_dir.display()
        )
    } else if !persist_mounted {
        format!(
            "Modelos em {} (Linux FS). Hotswap pedido se ok.",
            models_dir.display()
        )
    } else {
        "Downloads OK; hotswap não disparado (hotswap_now=false ou falha systemctl).".into()
    };

    // Downloads parciais NÃO bloqueiam a GUI: hotswap/cage é o gate de "transmute ok"
    // para o firstboot (senão cai no prompt bash sem tty7).
    let ok = if opts.hotswap_now {
        hotswap_triggered
    } else {
        errors.is_empty()
    };
    TransmuteRunResult {
        ok,
        locale: loc.raw_locale,
        models_dir: models_dir.display().to_string(),
        downloaded,
        errors,
        tts_greeting_played,
        hotswap_triggered,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_models_dir_linux_first_without_fat_env() {
        std::env::remove_var("NIXAIOS_USE_FAT_PERSIST");
        let dir = tempdir().unwrap();
        let (models, persist, root) = resolve_models_dir(dir.path());
        assert!(!persist);
        assert_eq!(models, dir.path().join("models"));
        assert_eq!(root, dir.path().display().to_string());
        assert!(models.is_dir());
    }

    #[test]
    fn resolve_models_dir_fat_opt_in_falls_back_without_mount() {
        std::env::set_var("NIXAIOS_USE_FAT_PERSIST", "1");
        let dir = tempdir().unwrap();
        let (models, persist, _) = resolve_models_dir(dir.path());
        std::env::remove_var("NIXAIOS_USE_FAT_PERSIST");
        // Sem /mnt/nixaios-log montado → Linux FS
        assert!(!persist);
        assert_eq!(models, dir.path().join("models"));
    }
}
