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
        // Modelos "thinking": o raciocínio vem separado (`reasoning_content`).
        // Não é resposta — mas conta para o diagnóstico (não mostramos tags).
        if let Some(t) = v
            .pointer("/choices/0/delta/reasoning_content")
            .and_then(|x| x.as_str())
            .or_else(|| {
                v.pointer("/choices/0/message/reasoning_content")
                    .and_then(|x| x.as_str())
            })
        {
            if !t.is_empty() {
                return Some(SseChunk::Reasoning(t.to_string()));
            }
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
pub enum SseChunk {
    Text(String),
    /// Raciocínio (não vai para o utilizador).
    Reasoning(String),
    Done,
}

pub struct LlamaServerBackend {
    base: String,
    _model: PathBuf,
    n_predict: u32,
    /// Chave do llama-server (spawn gera; externo pode vir do env).
    api_key: Option<String>,
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
        let api_key = std::env::var("NOSD_LLAMA_SERVER_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            _model: model,
            n_predict,
            api_key,
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
        let api_key = std::env::var("NOSD_LLAMA_SERVER_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        let mut cmd = Command::new(&bin);
        cmd.arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-m")
            .arg(&model)
            // Segurança: loopback + API key + sem Web UI.
            .arg("--api-key")
            .arg(&api_key)
            .arg("--no-webui")
            // 0.7-alpha2: sem `-c` fixo, o fit escolhe o maior contexto que
            // cabe na VRAM (e ajusta NGL quando não é forçado).
            // `NOSD_LLAMA_FIT_CTX` é só o mínimo (default 2048); um
            // `NOSD_LLAMA_CTX` explícito continua a mandar.
            .arg("--fit")
            .arg("on");
        let fit_ctx = std::env::var("NOSD_LLAMA_FIT_CTX")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| *n >= 512)
            .unwrap_or(2048);
        cmd.arg("--fit-ctx").arg(fit_ctx.to_string());
        if ngl > 0 {
            cmd.arg("-ngl").arg(ngl.to_string());
        }
        if let Ok(ctx) = std::env::var("NOSD_LLAMA_CTX") {
            if let Ok(n) = ctx.parse::<u32>() {
                if n > 0 {
                    cmd.arg("-c").arg(n.to_string());
                }
            }
        }
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(SwapError::Io)?;
        let base = format!("http://127.0.0.1:{port}");
        let auth = format!("Bearer {api_key}");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(resp) = ureq::get(&format!("{base}/health"))
                .set("Authorization", &auth)
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
            "llama-server spawned pid={:?} base={} model={} ngl={} api_key=set",
            child.id(),
            base,
            model.display(),
            ngl
        );
        Ok(Self {
            base,
            _model: model,
            n_predict: n_predict_from_env(),
            api_key: Some(api_key),
            cancel: Arc::new(AtomicBool::new(false)),
            child: parking_lot::Mutex::new(Some(child)),
        })
    }

    /// Endpoint de chat: aplica o **template do GGUF** (papéis system/user).
    /// Sem isto o Qwen responde em modo de continuação crua e o primeiro
    /// output é `<think>` — o chat "não faz nada" (observado em metal).
    fn chat_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base)
    }

    fn post_chat(
        &self,
        agent: &ureq::Agent,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<ureq::Response, SwapError> {
        let mut req = agent.post(url).set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        req.send_string(&body.to_string())
            .map_err(|_| SwapError::Infer("llama-server http"))
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
        // 64 era pouco: com raciocínio o modelo gastava tudo a pensar e o
        // utilizador via só `<think>`. 192 dá resposta curta + folga.
        .unwrap_or(192)
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
        // Prompt composto (persona/recall/user) -> papéis do template de chat.
        let (system, user) = nos_core::text::split_chat_prompt(prompt);
        let mut messages: Vec<serde_json::Value> = Vec::new();
        if !system.is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": system}));
        }
        messages.push(serde_json::json!({"role": "user", "content": user}));
        let body = serde_json::json!({
            "messages": messages,
            "max_tokens": self.n_predict,
            "stream": true,
            "temperature": 0.7,
            "top_p": 0.9,
            // Qwen3.x: sem isto o orçamento de tokens vai todo para <think>.
            "chat_template_kwargs": {"enable_thinking": false},
        });
        let url = self.chat_url();
        let agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(120))
            .build();
        let mut resp = self.post_chat(&agent, &url, &body)?;
        if !(200..300).contains(&resp.status()) {
            // Build antigo do llama-server pode recusar campos extra (400).
            // Repetir sem `chat_template_kwargs`/sampling em vez de morrer.
            warn!(
                target: "nosd",
                "llama-server chat status {} — a repetir sem chat_template_kwargs",
                resp.status()
            );
            let minimal = serde_json::json!({
                "messages": body.get("messages").cloned().unwrap_or_default(),
                "max_tokens": self.n_predict,
                "stream": true,
            });
            resp = self.post_chat(&agent, &url, &minimal)?;
            if !(200..300).contains(&resp.status()) {
                return Err(SwapError::Infer("llama-server bad status"));
            }
        }
        let mut reader = BufReader::new(resp.into_reader());
        let mut line = String::new();
        let mut out = String::new();
        let mut reasoning = String::new();
        while reader.read_line(&mut line).map_err(SwapError::Io)? > 0 {
            if self.cancel.load(Ordering::Relaxed) {
                warn!(target: "nosd", "llama-server stream cancelled");
                break;
            }
            if let Some(chunk) = sse_data_chunk(&line) {
                match chunk {
                    SseChunk::Text(t) => out.push_str(&t),
                    SseChunk::Reasoning(t) => reasoning.push_str(&t),
                    SseChunk::Done => break,
                }
            }
            line.clear();
        }
        // Servidores antigos deixam as tags no content: limpar sempre.
        let reply = nos_core::text::strip_think(&out);
        if !reply.is_empty() {
            return Ok(reply);
        }
        // Só raciocínio: devolver isso é melhor que mostrar tags cruas (era o
        // que o utilizador via: `JARBAS: <think>`), mas tem de ser registado.
        let thought = nos_core::text::strip_think(&reasoning);
        if !thought.is_empty() {
            warn!(
                target: "nosd",
                "modelo devolveu apenas raciocínio ({} chars) — resposta incompleta",
                thought.chars().count()
            );
            return Ok(thought);
        }
        Err(SwapError::Infer("llama-server empty stream"))
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
    fn sse_parse_openai_reasoning() {
        let line = r#"data: {"choices":[{"delta":{"reasoning_content":"a pensar"}}]}"#;
        assert_eq!(
            sse_data_chunk(line),
            Some(SseChunk::Reasoning("a pensar".into()))
        );
    }

    #[test]
    fn n_predict_default_is_usable() {
        std::env::remove_var("NOSD_LLAMA_N_PREDICT");
        assert_eq!(n_predict_from_env(), 192);
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
