//! Pipeline de Voz 100% Local — Whisper.cpp (STT) + Piper ONNX (TTS) + Locale dinâmico.
//! Respeita a linguagem configurada na inicialização gráfica (ex.: `pt_BR.UTF-8` → `pt` / `pt_BR-faber-medium`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;
use tracing::{info, warn};

/// Postura dos CLIs de voz (P.4) — pronto se binário existe; modelos são `ModelAbsent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceEnginePosture {
    EngineReady { stt_cli: bool, tts_cli: bool },
    EngineAbsent(&'static str),
}

/// Sondagem só de engine (PATH / env); não exige modelos ONNX/bin.
pub fn probe_voice_engines() -> VoiceEnginePosture {
    let stt = find_whisper_cli().is_some();
    let tts = find_piper_cli().is_some() || find_espeak_cli().is_some();
    if stt || tts {
        VoiceEnginePosture::EngineReady {
            stt_cli: stt,
            tts_cli: tts,
        }
    } else {
        VoiceEnginePosture::EngineAbsent("whisper-cli and piper/espeak")
    }
}

/// `voice.stt`/`voice.turn` honesty: transcript vazio **não** é sucesso.
///
/// O whisper devolve stdout vazio quando não há fala (ou só ruído). Reportar
/// `ok:true` com `text:""` faz o chamador acreditar que ouviu algo — o mesmo
/// defeito que `percent:0` num denominador vazio. Sem observação ⇒ erro
/// explícito (metal 2026-11-03: A5 devolveu `ok:true, text:""`).
pub fn transcript_or_error(text: String) -> Result<String, VoiceError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(VoiceError::Exec("stt empty transcript".into()));
    }
    Ok(trimmed.to_string())
}

/// WAV válido = header 44 B + pelo menos uma amostra.
///
/// `is_file()` sozinho mente: `tts_temp_path()` cria o ficheiro **antes** de
/// qualquer sintetizador correr, portanto um piper que falha deixava um WAV
/// de 0 B a passar por bom e o fallback espeak nunca disparava.
fn wav_ready(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.len() > 44).unwrap_or(false)
}

#[derive(Debug, Error)]
pub enum VoiceError {
    #[error("engine absent: {0}")]
    EngineAbsent(&'static str),
    #[error("model absent: {0}")]
    ModelAbsent(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("voice exec: {0}")]
    Exec(String),
}

/// Configuração de idioma detectada do NixOS (`/etc/locale.conf`, `NOSD_LOCALE`, `LANG`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleVoiceSpec {
    pub raw_locale: String,
    pub whisper_lang: String,
    pub piper_voice: String,
    pub piper_onnx_url: String,
    pub piper_json_url: String,
    pub whisper_model_file: String,
    pub whisper_model_url: String,
    pub greeting: String,
}

impl LocaleVoiceSpec {
    /// Detecta o idioma ativo do sistema (configurado na GUI inicial) ou `NOSD_LOCALE`.
    pub fn detect() -> Self {
        let raw = std::env::var("NOSD_LOCALE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(read_etc_locale)
            .or_else(|| std::env::var("LANG").ok())
            .or_else(|| std::env::var("LC_ALL").ok())
            .unwrap_or_else(|| "pt_BR.UTF-8".into());
        Self::from_locale_str(&raw)
    }

    pub fn from_locale_str(raw: &str) -> Self {
        let clean = raw.trim().trim_matches('"');
        let lower = clean.to_ascii_lowercase();
        let (whisper_lang, piper_voice, piper_rel, greeting) = if lower.starts_with("pt_pt") {
            (
                "pt",
                "pt_PT-tugao-medium",
                "pt/pt_PT/tugao/medium/pt_PT-tugao-medium",
                "Sistema Neural A I O S pronto. Cortex, Hermes e Jarbas ativos.",
            )
        } else if lower.starts_with("pt") {
            (
                "pt",
                "pt_BR-faber-medium",
                "pt/pt_BR/faber/medium/pt_BR-faber-medium",
                "Sistema Neural A I O S configurado em português. Cortex, Hermes e Jarbas online.",
            )
        } else if lower.starts_with("es") {
            (
                "es",
                "es_ES-davefx-medium",
                "es/es_ES/davefx/medium/es_ES-davefx-medium",
                "Sistema Neural A I O S configurado en español. Cortex, Hermes y Jarbas en línea.",
            )
        } else if lower.starts_with("fr") {
            (
                "fr",
                "fr_FR-siwis-medium",
                "fr/fr_FR/siwis/medium/fr_FR-siwis-medium",
                "Système Neural A I O S prêt. Cortex, Hermes et Jarbas en ligne.",
            )
        } else if lower.starts_with("de") {
            (
                "de",
                "de_DE-thorsten-medium",
                "de/de_DE/thorsten/medium/de_DE-thorsten-medium",
                "Neural A I O S System bereit. Cortex, Hermes und Jarbas sind online.",
            )
        } else {
            (
                "en",
                "en_US-lessac-medium",
                "en/en_US/lessac/medium/en_US-lessac-medium",
                "Neural A I O S system ready. Cortex, Hermes, and Jarbas are online.",
            )
        };

        let base_piper = "https://huggingface.co/rhasspy/piper-voices/resolve/main";
        Self {
            raw_locale: clean.to_string(),
            whisper_lang: whisper_lang.into(),
            piper_voice: piper_voice.into(),
            piper_onnx_url: format!("{base_piper}/{piper_rel}.onnx"),
            piper_json_url: format!("{base_piper}/{piper_rel}.onnx.json"),
            whisper_model_file: "ggml-base.bin".into(),
            whisper_model_url:
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
            greeting: greeting.into(),
        }
    }
}

fn read_etc_locale() -> Option<String> {
    // 1) Ficheiros explícitos do nosd / persistência / GNOME AccountsService
    // 0.5.0 Linux-first: sem preferência FAT (NIXAIOSLOG).
    for path in [
        "/var/lib/nosd/locale.conf",
        "/home/nixos/.config/gnome-initial-setup-locale",
        "/var/lib/AccountsService/users/nixos",
        "/etc/locale.conf",
        "/etc/default/locale",
    ] {
        if let Ok(txt) = fs::read_to_string(path) {
            for line in txt.lines() {
                let l = line.trim();
                for prefix in ["LANG=", "Language=", "FormatsLocale="] {
                    if let Some(v) = l.strip_prefix(prefix) {
                        let v = v.trim().trim_matches('"').trim_matches('\'');
                        if !v.is_empty() {
                            return Some(v.to_string());
                        }
                    }
                }
                // ficheiro puro de uma linha (ex. pt_BR.UTF-8)
                if !l.is_empty() && !l.contains('=') && !l.starts_with('[') && !l.starts_with('#') {
                    return Some(l.to_string());
                }
            }
        }
    }
    None
}

fn which(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for name in names {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn find_whisper_cli() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NOSD_WHISPER_CLI") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    which(&["whisper-cli", "whisper-cpp", "whisper"])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadStep {
    Wait,
    Start,
    Continue,
    Stop,
}

/// RMS de um bloco PCM s16LE mono.
pub fn rms_s16le(bytes: &[u8]) -> f32 {
    let n = bytes.len() / 2;
    if n == 0 {
        return 0.0;
    }
    let mut acc = 0.0f64;
    for i in 0..n {
        let s = i16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]) as f64;
        acc += s * s;
    }
    (acc / n as f64).sqrt() as f32
}

/// Máquina de estados do VAD (pura): 3 blocos acima para arrancar,
/// 6 abaixo (~600 ms) para parar.
pub fn vad_step(
    started: bool,
    rms: f32,
    threshold: f32,
    above_run: u32,
    below_run: u32,
) -> VadStep {
    if !started {
        if rms >= threshold && above_run >= 3 {
            VadStep::Start
        } else {
            VadStep::Wait
        }
    } else if rms < threshold && below_run >= 6 {
        VadStep::Stop
    } else {
        VadStep::Continue
    }
}

/// Configuração do backend `audio.cpp` (Audio8 TTS): binário **absoluto** +
/// GGUF. Variáveis: `NOSD_TTS_BIN`, `NOSD_TTS_MODEL`, `NOSD_TTS_BACKEND`.
///
/// Binário relativo é ignorado (o PATH não decide — lição 35).
fn audiocpp_config() -> Option<(PathBuf, String)> {
    audiocpp_config_from(
        std::env::var("NOSD_TTS_BIN").ok().as_deref(),
        std::env::var("NOSD_TTS_MODEL").ok().as_deref(),
    )
}

fn audiocpp_config_from(bin: Option<&str>, model: Option<&str>) -> Option<(PathBuf, String)> {
    let bin = bin.map(str::trim).filter(|s| !s.is_empty())?;
    let path = PathBuf::from(bin);
    if !path.is_absolute() {
        return None;
    }
    let model = model.map(str::trim).filter(|s| !s.is_empty())?;
    Some((path, model.to_string()))
}

pub fn find_piper_cli() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NOSD_PIPER_CLI") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // 0.5.0: Linux-first (sem FAT).
    let fixed = PathBuf::from("/var/lib/nosd/models/piper/piper");
    if fixed.is_file() {
        return Some(fixed);
    }
    which(&["piper"])
}

pub fn find_espeak_cli() -> Option<PathBuf> {
    which(&["espeak-ng", "espeak"])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceModelInfo {
    pub path: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// C4.1: o basename do `.onnx` tem de comecar pelo idioma primario do locale
/// (`pt`, `pt_BR` e `pt-br` casam com `pt`; `en_US` nao).
pub fn piper_model_matches_lang(file_stem: &str, lang: &str) -> bool {
    let stem = file_stem.to_ascii_lowercase();
    let lang = lang.to_ascii_lowercase();
    let primary = lang.split(['_', '-']).next().unwrap_or("").to_string();
    if primary.is_empty() {
        return false;
    }
    stem.starts_with(&primary)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceStatus {
    pub locale: LocaleVoiceSpec,
    pub whisper_cli: Option<String>,
    pub whisper_model: Option<String>,
    pub piper_cli: Option<String>,
    pub piper_model: Option<String>,
    pub stt_ready: bool,
    pub tts_ready: bool,
    /// Estágios honesty (ADR-0045 ADAPT): mic→vad→wake→stt→handle→tts→out.
    pub pipeline_stages: Vec<String>,
    /// Ready = STT+TTS; Absent = engines em falta (OK honesty, não capability).
    pub posture: String,
    /// Engines TTS disponíveis localmente (sem URLs).
    #[serde(default)]
    pub engines: Vec<String>,
    /// Modelos Piper .onnx sob models_dir (basename paths).
    #[serde(default)]
    pub models: Vec<VoiceModelInfo>,
    #[serde(default)]
    pub selected_engine: Option<String>,
    #[serde(default)]
    pub selected_model: Option<String>,
    /// Rótulo curto para HUD: piper|espeak|absent.
    #[serde(default)]
    pub tts_engine: String,
    /// C4.2: porque este modelo STT (base > tiny), declarado em vez de implicito.
    #[serde(default)]
    pub stt_model_reason: String,
}

#[derive(Clone)]
pub struct VoicePipeline {
    pub models_dir: PathBuf,
}

impl VoicePipeline {
    pub fn new() -> Self {
        Self::with_models_dir(PathBuf::from("/var/lib/nosd/models"))
    }

    pub fn with_models_dir(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

    pub fn engine_posture(&self) -> VoiceEnginePosture {
        probe_voice_engines()
    }

    pub fn status(&self) -> VoiceStatus {
        self.status_with_prefs(None, None)
    }

    /// `selected_engine` / `selected_model` vêm de ui_prefs (basename .onnx).
    pub fn status_with_prefs(
        &self,
        selected_engine: Option<&str>,
        selected_model: Option<&str>,
    ) -> VoiceStatus {
        let locale = LocaleVoiceSpec::detect();
        let w_cli = find_whisper_cli();
        let piper_cli = find_piper_cli();
        let espeak_cli = find_espeak_cli();
        let p_cli = piper_cli.clone().or_else(|| espeak_cli.clone());
        let w_model = self.find_whisper_model(&locale);
        let p_model = self.find_piper_model(&locale);
        let models = self.list_piper_models();
        let mut engines = Vec::new();
        if piper_cli.is_some() && (p_model.is_some() || !models.is_empty()) {
            engines.push("piper".into());
        }
        if espeak_cli.is_some() {
            engines.push("espeak".into());
        }
        let sel_eng = selected_engine
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| engines.iter().any(|e| e == s));
        let sel_model = selected_model
            .map(|s| s.to_string())
            .filter(|b| models.iter().any(|m| m.label == *b || m.path.ends_with(b)));
        let piper_ready = piper_cli.is_some() && (p_model.is_some() || sel_model.is_some());
        let stt_ready = w_cli.is_some() && w_model.is_some();
        let tts_ready = piper_ready || espeak_cli.is_some();
        let tts_engine: String = if sel_eng.as_deref() == Some("espeak") && espeak_cli.is_some() {
            "espeak".into()
        } else if piper_ready {
            "piper".into()
        } else if espeak_cli.is_some() {
            "espeak".into()
        } else {
            "absent".into()
        };
        let posture = if stt_ready && tts_ready {
            "Ready".into()
        } else {
            "EngineAbsent".into()
        };
        let stt_model_reason = match w_model
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|f| f.to_str())
        {
            Some("ggml-tiny.bin") => "tiny (embutido; base ausente - precisao menor)".to_string(),
            Some(name) if name.contains("base") => "base (preferido sobre tiny)".to_string(),
            Some(name) => format!("{name} (declarado pelo locale)"),
            None => "absent".to_string(),
        };
        VoiceStatus {
            stt_ready,
            tts_ready,
            stt_model_reason,
            locale,
            whisper_cli: w_cli.map(|p| p.display().to_string()),
            whisper_model: w_model.map(|p| p.display().to_string()),
            piper_cli: p_cli.map(|p| p.display().to_string()),
            piper_model: p_model.as_ref().map(|p| p.display().to_string()),
            pipeline_stages: ["mic", "vad", "wake", "stt", "handle", "tts", "out"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            posture,
            engines,
            models,
            selected_engine: sel_eng.or_else(|| {
                if tts_engine == "absent" {
                    None
                } else {
                    Some(tts_engine.clone())
                }
            }),
            selected_model: sel_model.or_else(|| {
                p_model
                    .as_ref()
                    .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            }),
            tts_engine,
        }
    }

    /// Lista `*.onnx` em models_dir (não recursivo profundo além de 1 nível).
    pub fn list_piper_models(&self) -> Vec<VoiceModelInfo> {
        let mut out = Vec::new();
        let mut push_file = |p: PathBuf| {
            if p.extension().and_then(|e| e.to_str()) != Some("onnx") {
                return;
            }
            let label = p
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            if label.is_empty() {
                return;
            }
            out.push(VoiceModelInfo {
                path: p.display().to_string(),
                label,
                sha256: None,
            });
        };
        if let Ok(rd) = fs::read_dir(&self.models_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file() {
                    push_file(p);
                } else if p.is_dir() {
                    if let Ok(sub) = fs::read_dir(&p) {
                        for s in sub.flatten() {
                            let sp = s.path();
                            if sp.is_file() {
                                push_file(sp);
                            }
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out.dedup_by(|a, b| a.label == b.label);
        out
    }

    fn find_whisper_model(&self, loc: &LocaleVoiceSpec) -> Option<PathBuf> {
        // C4.2: o tiny vinha primeiro por estar na ISO, e transcreve mal em fala
        // real. Ordem: modelo do locale, base, small e so no fim o tiny
        // embutido — a razao vai declarada em `stt_model_reason`.
        [
            self.models_dir.join(&loc.whisper_model_file),
            self.models_dir.join("ggml-base.bin"),
            self.models_dir.join("ggml-small.bin"),
            self.models_dir.join("ggml-tiny.bin"),
        ]
        .into_iter()
        .find(|cand| cand.is_file())
    }

    fn find_piper_model(&self, loc: &LocaleVoiceSpec) -> Option<PathBuf> {
        let preferred = self.models_dir.join(format!("{}.onnx", loc.piper_voice));
        if preferred.is_file() {
            return Some(preferred);
        }
        // C4.1: cair no primeiro `.onnx` punha uma voz estrangeira a falar em
        // silencio. So vale um modelo cujo basename seja do idioma pedido.
        let mut accepted: Vec<PathBuf> = Vec::new();
        if let Ok(rd) = fs::read_dir(&self.models_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("onnx") {
                    continue;
                }
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if piper_model_matches_lang(stem, &loc.whisper_lang) {
                    accepted.push(p);
                }
            }
        }
        accepted.sort();
        accepted.into_iter().next()
    }

    pub fn wake_word_poll(&self, pcm: &[i16]) -> bool {
        if pcm.is_empty() {
            return false;
        }
        let sum_sq: f64 = pcm.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (sum_sq / (pcm.len() as f64)).sqrt();
        rms > 1200.0
    }

    /// Transcreve ficheiro WAV 16kHz mono usando `whisper-cli` local + idioma do sistema.
    pub fn transcribe_wav_path(&self, wav_path: &Path) -> Result<String, VoiceError> {
        let loc = LocaleVoiceSpec::detect();
        let cli = find_whisper_cli().ok_or(VoiceError::EngineAbsent("whisper-cli"))?;
        let model = self
            .find_whisper_model(&loc)
            .ok_or_else(|| VoiceError::ModelAbsent(loc.whisper_model_file.clone()))?;

        let out = Command::new(&cli)
            .args([
                "-m",
                &model.display().to_string(),
                "-l",
                &loc.whisper_lang,
                "-f",
                &wav_path.display().to_string(),
                "--no-timestamps",
            ])
            .output()?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            warn!(target: "nosd", "whisper fail: {}", err);
            return Err(VoiceError::Exec(err.trim().into()));
        }

        let raw = String::from_utf8_lossy(&out.stdout);
        let text: String = raw
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('['))
            .collect::<Vec<_>>()
            .join(" ");
        info!(
            target: "nosd",
            "stt locale={} lang={} len={} text={}",
            loc.raw_locale,
            loc.whisper_lang,
            text.len(),
            text
        );
        Ok(text)
    }

    /// VAD + transcrição: escuta até haver fala e depois ~600 ms de silêncio
    /// (ou `max_secs`). Base do "loop contínuo" — devolve o transcript.
    pub fn record_and_transcribe_vad(&self, max_secs: u32) -> Result<String, VoiceError> {
        let max_secs = max_secs.clamp(2, 15);
        let arec = which(&["arecord"]).ok_or(VoiceError::EngineAbsent("arecord"))?;
        let mut child = Command::new(arec)
            .args(["-q", "-f", "S16_LE", "-r", "16000", "-c", "1", "-t", "raw"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut out = child
            .stdout
            .take()
            .ok_or(VoiceError::Exec("arecord: sem stdout".into()))?;
        use std::io::Read as _;

        let mut pcm: Vec<i16> = Vec::new();
        let mut buf = [0u8; 3200]; // 100 ms a 16 kHz mono s16
        let mut above = 0u32;
        let mut below = 0u32;
        let mut started = false;
        let mut noise: Vec<f32> = Vec::new();
        let mut threshold = 600.0f32;
        let max_frames = max_secs * 10;

        for _ in 0..max_frames {
            let n = out.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let chunk = &buf[..n];
            let rms = rms_s16le(chunk);
            // Calibração nos primeiros 500 ms (ruído de fundo).
            if !started && noise.len() < 5 {
                noise.push(rms);
                if noise.len() == 5 {
                    let avg = noise.iter().sum::<f32>() / 5.0;
                    threshold = (avg * 3.0).max(500.0);
                }
            }
            if rms >= threshold {
                above = above.saturating_add(1);
                below = 0;
            } else {
                below = below.saturating_add(1);
                above = 0;
            }
            match vad_step(started, rms, threshold, above, below) {
                VadStep::Start => started = true,
                VadStep::Stop => break,
                _ => {}
            }
            if started {
                for i in 0..n / 2 {
                    pcm.push(i16::from_le_bytes([chunk[2 * i], chunk[2 * i + 1]]));
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();

        if !started || pcm.len() < 1600 {
            return Err(VoiceError::Exec("vad: sem fala detectada".into()));
        }
        let tmp = std::env::temp_dir().join("nosd-mic-vad.wav");
        write_wav_16k_mono(&tmp, &pcm)?;
        self.transcribe_wav_path(&tmp)
    }

    /// Captura N segundos do microfone default (PipeWire/ALSA) e transcreve localmente.
    pub fn record_and_transcribe(&self, secs: u32) -> Result<String, VoiceError> {
        let secs = secs.clamp(1, 15);
        let tmp_wav = std::env::temp_dir().join("nosd-mic-capture.wav");
        let _ = fs::remove_file(&tmp_wav);

        let recorded = if which(&["arecord"]).is_some() {
            Command::new("arecord")
                .args([
                    "-q",
                    "-f",
                    "S16_LE",
                    "-r",
                    "16000",
                    "-c",
                    "1",
                    "-d",
                    &secs.to_string(),
                ])
                .arg(&tmp_wav)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else if which(&["pw-record"]).is_some() {
            Command::new("timeout")
                .arg(format!("{secs}s"))
                .arg("pw-record")
                .args(["--rate", "16000", "--channels", "1", "--format", "s16"])
                .arg(&tmp_wav)
                .status()
                .map(|s| s.success() || s.code() == Some(124))
                .unwrap_or(false)
        } else {
            return Err(VoiceError::EngineAbsent("arecord/pw-record"));
        };

        if !recorded || !tmp_wav.is_file() {
            return Err(VoiceError::Exec("mic capture failed".into()));
        }
        self.transcribe_wav_path(&tmp_wav)
    }

    pub fn transcribe(&self, pcm: &[i16]) -> Result<String, VoiceError> {
        if pcm.is_empty() {
            return Err(VoiceError::EngineAbsent("empty_pcm"));
        }
        let tmp_wav = std::env::temp_dir().join("nosd-pcm-in.wav");
        write_wav_16k_mono(&tmp_wav, pcm)?;
        self.transcribe_wav_path(&tmp_wav)
    }

    /// Sintetiza texto via `piper` ONNX local (ou fallback local `espeak-ng` no idioma) e toca via PipeWire/ALSA.
    /// Devolve `(wav_bytes, play_ok)` — `play_ok=false` se o pedido era tocar e aplay/pw-play falhou.
    pub fn synthesize_wav(
        &self,
        text: &str,
        play_audio: bool,
    ) -> Result<(Vec<u8>, bool), VoiceError> {
        let loc = LocaleVoiceSpec::detect();
        // F5: WAV único por chamada. Dois TTS concorrentes (UI + chat) não
        // podem trocar ficheiros; o guard apaga no fim do scope.
        let out_wav_guard = tts_temp_path()?;
        let out_wav = out_wav_guard.to_path_buf();

        // Backend de alta qualidade (`audio.cpp` / Audio8 GGUF), só quando o
        // operador configura caminho absoluto + modelo. Sem Python, sem CUDA
        // obrigatória (--backend cpu|vulkan|cuda). Off por omissão.
        if let Some((bin, model)) = audiocpp_config() {
            let backend = std::env::var("NOSD_TTS_BACKEND").unwrap_or_else(|_| "cpu".into());
            let out = Command::new(&bin)
                .args([
                    "--family",
                    "audio8_tts",
                    "--task",
                    "tts",
                    "--backend",
                    &backend,
                    "--model",
                    &model,
                    "--text",
                    text,
                    "--out",
                ])
                .arg(&out_wav)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()?;
            if !out.status.success() || !wav_ready(&out_wav) {
                warn!(
                    target: "nosd",
                    "audiocpp tts fail: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }

        // C4.3: se o audio.cpp ja escreveu um WAV valido, o piper nao corre —
        // dois escritores no mesmo ficheiro truncavam o bom resultado.
        if !wav_ready(&out_wav) {
            if let (Some(cli), Some(model)) = (find_piper_cli(), self.find_piper_model(&loc)) {
                let mut child = Command::new(&cli)
                    .args([
                        "--model",
                        &model.display().to_string(),
                        "--output_file",
                        &out_wav.display().to_string(),
                    ])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()?;

                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(text.as_bytes())?;
                    stdin.write_all(b"\n")?;
                }
                let output = child.wait_with_output()?;
                if !output.status.success() || !wav_ready(&out_wav) {
                    let err = String::from_utf8_lossy(&output.stderr);
                    warn!(target: "nosd", "piper fail: {}", err);
                }
            }
        }

        // Se o piper ainda não foi baixado (ou produziu WAV vazio), usa espeak-ng
        // local no idioma selecionado (ex. pt-br).
        if !wav_ready(&out_wav) {
            // C4.3: um WAV meio escrito passava por bom e o espeak escrevia por
            // cima; apagar primeiro garante um so autor por ficheiro.
            let _ = fs::remove_file(&out_wav);
            let espeak = find_espeak_cli().ok_or(VoiceError::EngineAbsent("piper/espeak-ng"))?;
            let voice_arg = match loc.whisper_lang.as_str() {
                "pt" => "pt-br",
                "es" => "es",
                "fr" => "fr",
                "de" => "de",
                _ => "en-us",
            };
            let st = Command::new(&espeak)
                .args(["-v", voice_arg, "-s", "155", "-w"])
                .arg(&out_wav)
                .arg(text)
                .status()?;
            if !st.success() || !wav_ready(&out_wav) {
                return Err(VoiceError::Exec("espeak-ng synthesis failed".into()));
            }
        }

        let mut play_ok = !play_audio;
        if play_audio {
            // Preferir PipeWire (mesmo XDG_RUNTIME_DIR do nosd/cage); ALSA
            // como fallback. Esperar o play — spawn+forget perdia erros e
            // cortava o TTS no Slint antes do fim.
            match play_wav_file(&out_wav) {
                Ok(()) => play_ok = true,
                Err(e) => {
                    play_ok = false;
                    warn!(target: "nosd", "tts play fail: {e}");
                }
            }
        }

        // ENOENT aqui significa: nenhum backend escreveu o WAV. Nomear o
        // caminho e o tamanho evita o "io: No such file or directory" opaco
        // que apareceu no metal (voice.tts, 2026-11-03).
        let bytes = fs::read(&out_wav).map_err(|e| {
            VoiceError::Exec(format!(
                "tts wav {} ({}): {e}",
                out_wav.display(),
                "nao escrito"
            ))
        })?;
        if bytes.len() <= 44 {
            return Err(VoiceError::Exec("tts: wav vazio (0 amostras)".into()));
        }
        info!(
            target: "nosd",
            "tts locale={} bytes={} play={} play_ok={}",
            loc.raw_locale,
            bytes.len(),
            play_audio,
            play_ok
        );
        Ok((bytes, play_ok))
    }

    pub fn synthesize(&self, text: &str) -> Result<Vec<i16>, VoiceError> {
        let (wav, _) = self.synthesize_wav(text, false)?;
        if wav.len() <= 44 {
            return Ok(Vec::new());
        }
        let pcm = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        Ok(pcm)
    }
}

impl Default for VoicePipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// WAV temporário único por chamada de TTS. O `TempPath` apaga o ficheiro no
/// drop; dois pedidos concorrentes nunca partilham o mesmo caminho.
fn tts_temp_path() -> std::io::Result<tempfile::TempPath> {
    tempfile::Builder::new()
        .prefix("nosd-tts-")
        .suffix(".wav")
        .tempfile()
        .map(|f| f.into_temp_path())
}

/// Reproduz WAV via PipeWire (`pw-play`) ou ALSA (`aplay`), com vários
/// devices — no metal o default pode ser HDMI mudo / sem sink PipeWire.
fn play_wav_file(path: &Path) -> Result<(), VoiceError> {
    if which(&["pw-play"]).is_some() {
        let st = Command::new("pw-play").arg(path).status()?;
        if st.success() {
            return Ok(());
        }
        warn!(target: "nosd", "pw-play exit={st:?}; fallback aplay");
    }
    let aplay = which(&["aplay"]).ok_or(VoiceError::EngineAbsent("pw-play/aplay"))?;
    // C4.4: devices fixos (`plughw:0,0`) falham quando o cartao 0 esta ocupado
    // ou e um HDMI mudo. Descobrir os cartoes reais do host e tentar todos.
    let mut tries: Vec<Vec<String>> = vec![
        vec!["-q".to_string()],
        vec!["-q".to_string(), "-D".to_string(), "pipewire".to_string()],
        vec!["-q".to_string(), "-D".to_string(), "default".to_string()],
    ];
    for (card, device) in aplay_cards(&aplay) {
        tries.push(vec![
            "-q".to_string(),
            "-D".to_string(),
            format!("plughw:{card},{device}"),
        ]);
    }
    for args in tries {
        let mut cmd = Command::new(&aplay);
        cmd.args(&args).arg(path);
        match cmd.status() {
            Ok(st) if st.success() => {
                info!(target: "nosd", "aplay ok args={args:?}");
                return Ok(());
            }
            Ok(st) => warn!(target: "nosd", "aplay exit={st:?} args={args:?}"),
            Err(e) => warn!(target: "nosd", "aplay err={e} args={args:?}"),
        }
    }
    Err(VoiceError::Exec("aplay: todos os devices falharam".into()))
}

/// Cartoes/device ALSA reais (`aplay -l`), na ordem do kernel (C4.4).
fn aplay_cards(aplay: &Path) -> Vec<(u32, u32)> {
    match Command::new(aplay).arg("-l").output() {
        Ok(out) => parse_aplay_cards(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => Vec::new(),
    }
}

/// Parser puro de `aplay -l` (testavel sem placa de som).
fn parse_aplay_cards(text: &str) -> Vec<(u32, u32)> {
    let mut cards: Vec<(u32, u32)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("card ") else {
            continue;
        };
        let Some((card_part, dev_part)) = rest.split_once(", device ") else {
            continue;
        };
        let card = card_part
            .split(':')
            .next()
            .and_then(|n| n.trim().parse::<u32>().ok());
        let device = dev_part
            .split(':')
            .next()
            .and_then(|n| n.trim().parse::<u32>().ok());
        if let (Some(c), Some(d)) = (card, device) {
            if !cards.contains(&(c, d)) {
                cards.push((c, d));
            }
        }
    }
    cards
}

fn write_wav_16k_mono(path: &Path, pcm: &[i16]) -> std::io::Result<()> {
    let data_len = (pcm.len() * 2) as u32;
    let riff_len = 36 + data_len;
    let mut f = fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&riff_len.to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&16000u32.to_le_bytes())?;
    f.write_all(&32000u32.to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &s in pcm {
        f.write_all(&s.to_le_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod vad_tests {
    use super::{rms_s16le, vad_step, VadStep};

    #[test]
    fn rms_silence_and_signal() {
        assert_eq!(rms_s16le(&[]), 0.0);
        assert_eq!(rms_s16le(&[0, 0, 0, 0]), 0.0);
        // 0x4000 = 16384 → RMS 16384
        assert!(rms_s16le(&[0x00, 0x40, 0x00, 0x40]) > 16000.0);
    }

    #[test]
    fn vad_starts_after_three_loud_blocks() {
        assert_eq!(vad_step(false, 900.0, 500.0, 2, 0), VadStep::Wait);
        assert_eq!(vad_step(false, 900.0, 500.0, 3, 0), VadStep::Start);
        // ruído fraco nunca arranca, independentemente do contador
        assert_eq!(vad_step(false, 100.0, 500.0, 9, 0), VadStep::Wait);
    }

    #[test]
    fn vad_stops_after_six_quiet_blocks() {
        assert_eq!(vad_step(true, 100.0, 500.0, 0, 5), VadStep::Continue);
        assert_eq!(vad_step(true, 100.0, 500.0, 0, 6), VadStep::Stop);
    }

    #[test]
    fn vad_keeps_running_while_loud() {
        assert_eq!(vad_step(true, 900.0, 500.0, 9, 0), VadStep::Continue);
    }
}

#[cfg(test)]
mod tts_backend_tests {
    use super::audiocpp_config_from;

    #[test]
    fn audiocpp_requires_absolute_bin_and_model() {
        assert!(audiocpp_config_from(None, Some("/m/a.gguf")).is_none());
        assert!(audiocpp_config_from(Some("  "), Some("/m/a.gguf")).is_none());
        assert!(audiocpp_config_from(Some("audiocpp_cli"), Some("/m/a.gguf")).is_none());
        assert!(audiocpp_config_from(Some("/b/audiocpp_cli"), None).is_none());
        assert!(audiocpp_config_from(Some("/b/audiocpp_cli"), Some("  ")).is_none());
    }

    #[test]
    fn audiocpp_accepts_absolute_pair() {
        // `temp_dir()` é absoluto em Unix e Windows (ao contrário de "/b/…").
        let bin = std::env::temp_dir().join("audiocpp_cli");
        let bin_s = bin.to_str().expect("utf8");
        let got = audiocpp_config_from(Some(bin_s), Some("/m/a.gguf")).expect("config");
        assert_eq!(got.0, bin);
        assert_eq!(got.1, "/m/a.gguf");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tts_temp_paths_are_unique_and_removed() {
        let a = tts_temp_path().unwrap();
        let b = tts_temp_path().unwrap();
        assert_ne!(
            a.to_path_buf(),
            b.to_path_buf(),
            "cada chamada de TTS tem o seu WAV"
        );
        assert!(a.exists() && b.exists());
        let pa = a.to_path_buf();
        let pb = b.to_path_buf();
        drop(a);
        drop(b);
        assert!(!pa.exists(), "o guard apaga o WAV de A");
        assert!(!pb.exists(), "o guard apaga o WAV de B");
    }

    #[test]
    fn empty_transcript_is_an_error() {
        assert!(matches!(
            transcript_or_error("   ".into()),
            Err(VoiceError::Exec(_))
        ));
        assert!(matches!(
            transcript_or_error("".into()),
            Err(VoiceError::Exec(_))
        ));
        assert_eq!(transcript_or_error(" oi ".into()).unwrap(), "oi");
    }

    #[test]
    fn empty_wav_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("empty.wav");
        std::fs::write(&p, b"RIFF").unwrap();
        assert!(!wav_ready(&p), "ficheiro criado mas sem amostras nao conta");
        std::fs::write(&p, vec![0u8; 45]).unwrap();
        assert!(wav_ready(&p));
        assert!(!wav_ready(&dir.path().join("nope.wav")));
    }

    #[test]
    fn absent_honest() {
        let v = VoicePipeline::with_models_dir(PathBuf::from("/nonexistent/models"));
        assert!(matches!(
            v.transcribe(&[]),
            Err(VoiceError::EngineAbsent(_))
        ));
    }

    #[test]
    fn engine_ready_with_fake_cli_env() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("whisper-cli");
        std::fs::write(&fake, b"").unwrap();
        std::env::set_var("NOSD_WHISPER_CLI", fake.display().to_string());
        assert!(matches!(
            probe_voice_engines(),
            VoiceEnginePosture::EngineReady { stt_cli: true, .. }
        ));
        std::env::remove_var("NOSD_WHISPER_CLI");
    }

    #[test]
    fn locale_mapping_pt_and_en() {
        let pt = LocaleVoiceSpec::from_locale_str("pt_BR.UTF-8");
        assert_eq!(pt.whisper_lang, "pt");
        assert_eq!(pt.piper_voice, "pt_BR-faber-medium");

        let en = LocaleVoiceSpec::from_locale_str("en_US.UTF-8");
        assert_eq!(en.whisper_lang, "en");
        assert_eq!(en.piper_voice, "en_US-lessac-medium");
    }
}

#[cfg(test)]
mod f4_tests {
    use super::*;

    #[test]
    fn piper_model_must_match_the_language() {
        assert!(piper_model_matches_lang("pt_BR-faber-low", "pt"));
        assert!(piper_model_matches_lang("pt-br-faber", "pt_BR"));
        assert!(!piper_model_matches_lang("en_US-amy-low", "pt"));
        assert!(!piper_model_matches_lang("en_US-amy-low", "pt_BR"));
        assert!(!piper_model_matches_lang("pt_BR-faber", ""));
    }

    #[test]
    fn aplay_list_parses_real_cards() {
        let sample = "**** List of PLAYBACK Hardware Devices ****\ncard 0: PCH [HDA Intel PCH], device 0: ALC257 Analog [ALC257 Analog]\n  Subdevices: 1/1\ncard 0: PCH [HDA Intel PCH], device 3: HDMI 0 [HDMI 0]\ncard 1: Headset [Logitech Headset], device 0: USB Audio [USB Audio]\n";
        assert_eq!(parse_aplay_cards(sample), vec![(0, 0), (0, 3), (1, 0)]);
        assert!(parse_aplay_cards("").is_empty());
    }
}
