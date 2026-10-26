//! HTTP client opcional para `llama-server` (`NOSD_LLAMA_SERVER=http://127.0.0.1:8080`).
//!
//! Stream SSE drenado; cancel via `Drop` + `AtomicBool`. E2E runtime NOT verificado aqui.

use super::backend::{InferenceBackend, SwapError};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub fn llama_server_base() -> Option<String> {
    std::env::var("NOSD_LLAMA_SERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
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
            .or_else(|| v.pointer("/choices/0/delta/content").and_then(|x| x.as_str()))
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
}

impl LlamaServerBackend {
    pub fn try_new(model: PathBuf) -> Result<Self, SwapError> {
        if !model.is_file() {
            return Err(SwapError::Infer("gguf missing"));
        }
        let base = llama_server_base().ok_or(SwapError::EngineAbsent("llama-server url"))?;
        let n_predict = std::env::var("NOSD_LLAMA_N_PREDICT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(64);
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
        })
    }

    fn completion_url(&self) -> String {
        format!("{}/completion", self.base)
    }
}

impl Drop for LlamaServerBackend {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
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
        let agent = ureq::AgentBuilder::new().timeout_read(Duration::from_secs(120)).build();
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
        assert_eq!(
            sse_data_chunk(line),
            Some(SseChunk::Text(" hello".into()))
        );
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
}
