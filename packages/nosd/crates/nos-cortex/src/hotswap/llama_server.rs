//! HTTP client opcional para `llama-server` (`NOSD_LLAMA_SERVER=http://127.0.0.1:8080`).
//!
//! Stream SSE drenado; cancel via `Drop` + `AtomicBool`. E2E runtime NOT verificado aqui.

use super::backend::{validate_gguf, InferenceBackend, SwapError};
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

pub fn llama_server_base() -> Option<String> {
    std::env::var("NOSD_LLAMA_SERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Honesty S2.2: `absent` | `url_set` (env only — e2e daemon metal = residual).
pub fn llama_server_posture(engine_name: &str) -> String {
    if engine_name == "llama-server-http" {
        return format!("managed+engine={engine_name}");
    }
    match llama_server_base() {
        Some(_) => format!("url_set+engine={engine_name}"),
        None => format!("absent+engine={engine_name}"),
    }
}

/// Extrai delta de texto de linha SSE `data: {...}` ou `data: [DONE]`.
pub fn sse_data_chunk(line: &str) -> Option<SseChunk> {
    let line = line.trim();
    let payload = line.strip_prefix("data:")?.trim();
    if payload == "[DONE]" {
        return Some(SseChunk::Done);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
        if let Some(t) = v
            .pointer("/content")
            .and_then(|x| x.as_str())
            .or_else(|| {
                v.pointer("/choices/0/delta/content")
                    .and_then(|x| x.as_str())
            })
            .or_else(|| v.pointer("/choices/0/text").and_then(|x| x.as_str()))
        {
            if !t.is_empty() {
                return Some(SseChunk::Text(t.to_string()));
            }
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
pub enum SseChunk {
    Text(String),
    Done,
}

pub struct LlamaServerBackend {
    base: String,
    _model: PathBuf,
    n_predict: u32,
    cancel: Arc<AtomicBool>,
    /// Child próprio (spawn) — None quando é um servidor externo.
    child: parking_lot::Mutex<Option<Child>>,
}

impl LlamaServerBackend {
    pub fn try_new(model: PathBuf) -> Result<Self, SwapError> {
        let base = llama_server_base().ok_or(SwapError::EngineAbsent("llama-server url"))?;
        Self::try_new_at(model, base)
    }

    /// Anexa a um llama-server já existente (env `NOSD_LLAMA_SERVER`).
    pub fn try_new_at(model: PathBuf, base: String) -> Result<Self, SwapError> {
        if !model.is_file() {
            return Err(SwapError::Infer("gguf missing"));
        }
        validate_gguf(&model)?;
        let n_predict = n_predict_from_env();
        info!(
            target: "nosd",
            "llama-server backend base={} model={}",
            base,
            model.display()
        );
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            _model: model,
            n_predict,
            cancel: Arc::new(AtomicBool::new(false)),
            child: parking_lot::Mutex::new(None),
        })
    }

    /// 0.6: sobe um `llama-server` persistente e espera `/health`.
    /// Sem binário/timeout → `EngineAbsent` (caller faz fallback cli).
    pub fn spawn(model: PathBuf) -> Result<Self, SwapError> {
        if !model.is_file() {
            return Err(SwapError::Infer("gguf missing"));
        }
        validate_gguf(&model)?;
        let bin = find_bin().ok_or(SwapError::EngineAbsent("llama-server bin"))?;
        let port = pick_port()?;
        let ngl = ngl_from_env();
        let mut cmd = Command::new(&bin);
        cmd.arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-m")
            .arg(&model);
        if ngl > 0 {
            cmd.arg("-ngl").arg(ngl.to_string());
        }
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(SwapError::Io)?;
        let base = format!("http://127.0.0.1:{port}");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(resp) = ureq::get(&format!("{base}/health"))
                .timeout(Duration::from_secs(2))
                .call()
            {
                if (200..300).contains(&resp.status()) {
                    break;
                }
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SwapError::EngineAbsent("llama-server health timeout"));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        info!(
            target: "nosd",
            "llama-server spawned pid={:?} base={} model={} ngl={}",
            child.id(),
            base,
            model.display(),
            ngl
        );
        Ok(Self {
            base,
            _model: model,
            n_predict: n_predict_from_env(),
            cancel: Arc::new(AtomicBool::new(false)),
            child: parking_lot::Mutex::new(Some(child)),
        })
    }

    fn completion_url(&self) -> String {
        format!("{}/completion", self.base)
    }
}

impl Drop for LlamaServerBackend {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn n_predict_from_env() -> u32 {
    std::env::var("NOSD_LLAMA_N_PREDICT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64)
}

fn ngl_from_env() -> u32 {
    std::env::var("NOSD_LLAMA_NGL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn pick_port() -> Result<u16, SwapError> {
    if let Ok(p) = std::env::var("NOSD_LLAMA_SERVER_PORT") {
        if let Ok(port) = p.parse::<u16>() {
            return Ok(port);
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(SwapError::Io)?;
    let port = listener.local_addr().map_err(SwapError::Io)?.port();
    drop(listener);
    Ok(port)
}

fn find_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NOSD_LLAMA_SERVER_BIN") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    which("llama-server")
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

impl InferenceBackend for LlamaServerBackend {
    fn name(&self) -> &'static str {
        "llama-server-http"
    }

    fn infer(&self, prompt: &str) -> Result<String, SwapError> {
        self.cancel.store(false, Ordering::Relaxed);
        let body = serde_json::json!({
            "prompt": prompt,
            "n_predict": self.n_predict,
            "stream": true,
        });
        let url = self.completion_url();
        let agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(120))
            .build();
        let resp = agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(|_| SwapError::Infer("llama-server http"))?;
        if !(200..300).contains(&resp.status()) {
            return Err(SwapError::Infer("llama-server bad status"));
        }
        let mut reader = BufReader::new(resp.into_reader());
        let mut line = String::new();
        let mut out = String::new();
        while reader.read_line(&mut line).map_err(SwapError::Io)? > 0 {
            if self.cancel.load(Ordering::Relaxed) {
                warn!(target: "nosd", "llama-server stream cancelled");
                break;
            }
            if let Some(chunk) = sse_data_chunk(&line) {
                match chunk {
                    SseChunk::Text(t) => out.push_str(&t),
                    SseChunk::Done => break,
                }
            }
            line.clear();
        }
        if out.trim().is_empty() {
            return Err(SwapError::Infer("llama-server empty stream"));
        }
        Ok(out.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parse_completion_json() {
        let line = r#"data: {"content":" hello"}"#;
        assert_eq!(sse_data_chunk(line), Some(SseChunk::Text(" hello".into())));
    }

    #[test]
    fn sse_parse_openai_delta() {
        let line = r#"data: {"choices":[{"delta":{"content":"x"}}]}"#;
        assert_eq!(sse_data_chunk(line), Some(SseChunk::Text("x".into())));
    }

    #[test]
    fn sse_done() {
        assert_eq!(sse_data_chunk("data: [DONE]"), Some(SseChunk::Done));
    }

    #[test]
    fn posture_absent_without_env() {
        std::env::remove_var("NOSD_LLAMA_SERVER");
        let p = llama_server_posture("cpu-echo");
        assert!(p.starts_with("absent+"), "{p}");
        assert!(p.contains("cpu-echo"));
    }
}
