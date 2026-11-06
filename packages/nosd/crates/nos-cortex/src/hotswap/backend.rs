use std::fmt;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    CpuEcho,
    LlamaGguf,
    Exl2,
    VllmCuda,
    VllmRocm,
    LlamaHip,
    Ipex,
    OpenVino,
    LlamaSycl,
}

#[derive(Debug)]
pub enum SwapError {
    EngineAbsent(&'static str),
    /// Arrancou mas nao ficou saudavel (ex.: health timeout do llama-server).
    /// Distinto de `EngineAbsent` de proposito: quem chama **nao** deve
    /// degradar para um motor pior (llama-cli) sem o saber (C1.4).
    Boot(&'static str),
    Infer(&'static str),
    /// Resposta HTTP nao-2xx com o codigo e o corpo (diagnostico), em vez de
    /// um `Infer("http")` que perdia ambos (C3.1).
    Http {
        code: u16,
        body: String,
    },
    Io(std::io::Error),
}

impl fmt::Display for SwapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineAbsent(s) => write!(f, "engine absent: {s}"),
            Self::Boot(s) => write!(f, "engine boot: {s}"),
            Self::Infer(s) => write!(f, "infer: {s}"),
            Self::Http { code, body } => write!(f, "http {code}: {body}"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for SwapError {}

pub trait InferenceBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn infer(&self, prompt: &str) -> Result<String, SwapError>;
}

/// E-04: valida magic/versão/contadores do GGUF antes de attach.
pub fn validate_gguf(path: &Path) -> Result<(), SwapError> {
    let mut f = std::fs::File::open(path).map_err(SwapError::Io)?;
    let mut buf = [0u8; 24];
    f.read_exact(&mut buf).map_err(SwapError::Io)?;
    nos_core::parse_gguf_header(&buf)
        .map(|_| ())
        .map_err(|_| SwapError::Infer("invalid gguf header"))
}

pub struct CpuEchoBackend;

impl InferenceBackend for CpuEchoBackend {
    fn name(&self) -> &'static str {
        "cpu-echo"
    }

    fn infer(&self, prompt: &str) -> Result<String, SwapError> {
        Ok(format!(
            "[cpu-echo] {}",
            prompt.chars().take(256).collect::<String>()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn gguf_header_valid_and_invalid() {
        let dir = tempdir().unwrap();
        let valid = dir.path().join("ok.gguf");
        let mut f = std::fs::File::create(&valid).unwrap();
        f.write_all(b"GGUF").unwrap();
        f.write_all(&3u32.to_le_bytes()).unwrap();
        f.write_all(&0u64.to_le_bytes()).unwrap();
        f.write_all(&0u64.to_le_bytes()).unwrap();
        drop(f);
        assert!(validate_gguf(&valid).is_ok());

        let bad = dir.path().join("bad.gguf");
        std::fs::write(&bad, b"NOPE").unwrap();
        assert!(validate_gguf(&bad).is_err());
    }
}
