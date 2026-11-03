//! Preferências UI do kiosk — `<data-dir>/ui_prefs.json` (0600).
//! `prefs.set` = Cap NONE + allowlist de chaves (fail-closed).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const PREFS_FILE: &str = "ui_prefs.json";

/// Chaves permitidas em `prefs.set` (só estas).
pub const ALLOWED_KEYS: &[&str] = &[
    "volume",
    "muted",
    "sink_id",
    "source_id",
    "mic_default_on",
    "voice_engine",
    "voice_model",
    "idle_ms",
    "dpms_ms",
    "auto_voice",
    "vad_thr",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPrefs {
    #[serde(default = "default_volume")]
    pub volume: u32,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub sink_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub mic_default_on: bool,
    #[serde(default)]
    pub voice_engine: Option<String>,
    #[serde(default)]
    pub voice_model: Option<String>,
    #[serde(default = "default_idle_ms")]
    pub idle_ms: u64,
    #[serde(default = "default_dpms_ms")]
    pub dpms_ms: u64,
    #[serde(default)]
    pub auto_voice: bool,
    #[serde(default = "default_vad_thr")]
    pub vad_thr: f64,
}

fn default_volume() -> u32 {
    55
}
fn default_idle_ms() -> u64 {
    5 * 60 * 1000
}
fn default_dpms_ms() -> u64 {
    10 * 60 * 1000
}
fn default_vad_thr() -> f64 {
    0.04
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            volume: default_volume(),
            muted: false,
            sink_id: None,
            source_id: None,
            mic_default_on: false,
            voice_engine: None,
            voice_model: None,
            idle_ms: default_idle_ms(),
            dpms_ms: default_dpms_ms(),
            auto_voice: false,
            vad_thr: default_vad_thr(),
        }
    }
}

pub fn prefs_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PREFS_FILE)
}

pub fn load(data_dir: &Path) -> UiPrefs {
    let path = prefs_path(data_dir);
    let Ok(raw) = fs::read_to_string(&path) else {
        return UiPrefs::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(data_dir: &Path, prefs: &UiPrefs) -> Result<(), String> {
    let path = prefs_path(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&body).map_err(|e| e.to_string())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Merge allowlisted keys from JSON object into prefs; reject unknown keys / path-like values.
pub fn merge_patch(prefs: &mut UiPrefs, patch: &Map<String, Value>) -> Result<(), String> {
    for (k, v) in patch {
        if !ALLOWED_KEYS.contains(&k.as_str()) {
            return Err(format!("prefs.set: chave '{k}' nao permitida"));
        }
        match k.as_str() {
            "volume" => {
                let n = v.as_u64().ok_or("volume: u64")?;
                prefs.volume = (n as u32).min(150);
            }
            "muted" => prefs.muted = v.as_bool().ok_or("muted: bool")?,
            "sink_id" => prefs.sink_id = opt_id(v)?,
            "source_id" => prefs.source_id = opt_id(v)?,
            "mic_default_on" => prefs.mic_default_on = v.as_bool().ok_or("mic_default_on: bool")?,
            "voice_engine" => prefs.voice_engine = opt_engine(v)?,
            "voice_model" => prefs.voice_model = opt_model_path(v)?,
            "idle_ms" => prefs.idle_ms = v.as_u64().ok_or("idle_ms: u64")?.max(30_000),
            "dpms_ms" => prefs.dpms_ms = v.as_u64().ok_or("dpms_ms: u64")?.max(60_000),
            "auto_voice" => prefs.auto_voice = v.as_bool().ok_or("auto_voice: bool")?,
            "vad_thr" => {
                let f = v.as_f64().ok_or("vad_thr: f64")?;
                prefs.vad_thr = f.clamp(0.001, 1.0);
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn opt_id(v: &Value) -> Result<Option<String>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let s = v.as_str().ok_or("id: string")?.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if s.contains('/') || s.contains('\\') || s.contains("..") {
        return Err("id: charset".into());
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("id: charset".into());
    }
    Ok(Some(s.to_string()))
}

fn opt_engine(v: &Value) -> Result<Option<String>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let s = v.as_str().ok_or("voice_engine: string")?.trim().to_ascii_lowercase();
    match s.as_str() {
        "" => Ok(None),
        "piper" | "espeak" => Ok(Some(s)),
        _ => Err("voice_engine: piper|espeak".into()),
    }
}

fn opt_model_path(v: &Value) -> Result<Option<String>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let s = v.as_str().ok_or("voice_model: string")?.trim();
    if s.is_empty() {
        return Ok(None);
    }
    // Só basename .onnx — sem URLs / path traversal.
    if s.contains("://") || s.contains("..") || s.contains('\\') || s.starts_with('/') {
        return Err("voice_model: basename .onnx only".into());
    }
    if !s.ends_with(".onnx") || s.len() > 128 {
        return Err("voice_model: *.onnx basename".into());
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("voice_model: charset".into());
    }
    Ok(Some(s.to_string()))
}

pub fn to_json(prefs: &UiPrefs) -> Value {
    serde_json::to_value(prefs).unwrap_or(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reject_unknown_key() {
        let mut p = UiPrefs::default();
        let mut m = Map::new();
        m.insert("evil".into(), json!(1));
        assert!(merge_patch(&mut p, &m).is_err());
    }

    #[test]
    fn roundtrip_volume() {
        let dir = tempdir().unwrap();
        let mut p = UiPrefs::default();
        let mut m = Map::new();
        m.insert("volume".into(), json!(80));
        merge_patch(&mut p, &m).unwrap();
        save(dir.path(), &p).unwrap();
        let loaded = load(dir.path());
        assert_eq!(loaded.volume, 80);
    }

    #[test]
    fn reject_url_model() {
        let mut p = UiPrefs::default();
        let mut m = Map::new();
        m.insert("voice_model".into(), json!("https://evil/x.onnx"));
        assert!(merge_patch(&mut p, &m).is_err());
    }
}
