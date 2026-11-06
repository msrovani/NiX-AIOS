//! HotSwap multi-vendor — 4 fases; MVP CpuEcho + LlamaGguf(cli); vendors EngineAbsent.

mod backend;
mod llama_cli;
mod llama_server;

pub use backend::{CpuEchoBackend, EngineKind, InferenceBackend, SwapError};
pub use llama_cli::{find_llama_cli, LlamaCliBackend};
pub use llama_server::{
    llama_server_base, llama_server_posture, sse_data_chunk, LlamaServerBackend, SseChunk,
};

/// C1.3: `ensure_model_loaded` deixa de devolver um booleano mudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadOutcome {
    /// O modelo pedido ja estava activo (sem unload, sem spawn).
    Reused,
    /// Houve unload + attach (o motor anterior foi substituido).
    Swapped,
}

impl LoadOutcome {
    pub fn swapped(self) -> bool {
        matches!(self, Self::Swapped)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reused => "reused",
            Self::Swapped => "swapped",
        }
    }
}

/// Orcamento de tokens por resposta — um so valor para servidor **e** CLI.
/// C3.2: 64 no CLI contra 192 no servidor fazia o fallback responder a meio.
pub const DEFAULT_N_PREDICT: u32 = 192;

/// Cap de camadas pedido pelo observador (termico/presenca).
/// `u32::MAX` = sem cap. Antes vivia em `NOSD_LLAMA_NGL` (env), o que anulava
/// o `--fit` no spawn seguinte e nunca voltava a subir (C1.1/C3.3).
static THERMAL_NGL_CAP: AtomicU32 = AtomicU32::new(u32::MAX);

pub fn set_thermal_ngl_cap(cap: Option<u32>) {
    THERMAL_NGL_CAP.store(cap.unwrap_or(u32::MAX), Ordering::Relaxed);
}

pub fn thermal_ngl_cap() -> Option<u32> {
    let v = THERMAL_NGL_CAP.load(Ordering::Relaxed);
    (v != u32::MAX).then_some(v)
}

/// Aplica o cap **so** a um `-ngl` explicito: sem `-ngl` quem decide e o
/// `--fit`, e o cap fica so no log. Devolve `(ngl_efectivo, cap_visto)`.
pub fn apply_thermal_cap(requested: u32, cap: Option<u32>) -> (u32, Option<u32>) {
    match cap {
        Some(c) if requested > 0 => (requested.min(c), Some(c)),
        Some(c) => (0, Some(c)),
        None => (requested, None),
    }
}

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Clone)]
pub struct HotSwapController {
    active: Arc<Mutex<Arc<dyn InferenceBackend>>>,
    kind: Arc<Mutex<EngineKind>>,
    current_path: Arc<Mutex<Option<PathBuf>>>,
}

impl HotSwapController {
    pub fn new_cpu() -> Self {
        Self {
            active: Arc::new(Mutex::new(Arc::new(CpuEchoBackend))),
            kind: Arc::new(Mutex::new(EngineKind::CpuEcho)),
            current_path: Arc::new(Mutex::new(None)),
        }
    }

    pub fn current_model_path(&self) -> Option<PathBuf> {
        self.current_path.lock().clone()
    }

    /// MoE LOO: garante que o GGUF do slot roteado está ativo na dGPU/llama-cli.
    /// C1.3: devolve o que aconteceu (`Reused` vs `Swapped`) — o booleano antigo
    /// era `true` quando havia substituicao e o log lia-o como "carregado".
    pub fn ensure_model_loaded(&self, model: PathBuf) -> Result<LoadOutcome, SwapError> {
        if self.current_path.lock().as_ref() == Some(&model)
            && *self.kind.lock() == EngineKind::LlamaGguf
        {
            return Ok(LoadOutcome::Reused);
        }
        self.hot_load_llama(model)?;
        Ok(LoadOutcome::Swapped)
    }

    pub fn kind(&self) -> EngineKind {
        *self.kind.lock()
    }

    pub fn engine_name(&self) -> &'static str {
        self.active.lock().name()
    }

    pub fn infer(&self, prompt: &str) -> Result<String, SwapError> {
        // E-02: não segurar o lock durante IO/subprocess.
        let backend = self.active.lock().clone();
        let start = std::time::Instant::now();
        let out = backend.infer(prompt);
        info!(
            target: "nosd",
            "infer engine={} latency_ms={} ok={}",
            backend.name(),
            start.elapsed().as_millis(),
            out.is_ok()
        );
        out
    }

    /// Descarrega engine atual → CpuEcho (libera subprocess llama / VRAM do filho).
    pub fn unload(&self) {
        info!(target: "nosd", "hotswap unload → cpu-echo (was {})", self.engine_name());
        *self.active.lock() = Arc::new(CpuEchoBackend);
        *self.kind.lock() = EngineKind::CpuEcho;
        *self.current_path.lock() = None;
    }

    /// Tenta anexar GGUF: `llama-server` externo (env), `llama-server` spawned
    /// (0.6 default) ou `llama-cli` fallback. Falha → mantém CpuEcho.
    pub fn try_attach_llama(&self, model: PathBuf) -> Result<(), SwapError> {
        let backend: Arc<dyn InferenceBackend> = if let Some(base) = llama_server_base() {
            Arc::new(LlamaServerBackend::try_new_at(model.clone(), base)?)
        } else {
            match LlamaServerBackend::spawn(model.clone()) {
                Ok(b) => Arc::new(b),
                // Binario ausente: o CLI e um downgrade legitimo e declarado.
                Err(e @ SwapError::EngineAbsent(_)) => {
                    info!(
                        target: "nosd",
                        "llama-server spawn unavailable ({e}) - llama-cli fallback"
                    );
                    Arc::new(LlamaCliBackend::try_new(model.clone())?)
                }
                // Arrancou e nao ficou saudavel (health timeout): nao trocar por
                // um motor com metade do orcamento de tokens atras das costas de
                // quem chamou (C1.4) — quem chamou decide (S1 -> cpu-echo).
                Err(e) => return Err(e),
            }
        };
        info!(target: "nosd", "hotswap attach {}", backend.name());
        *self.active.lock() = backend;
        *self.kind.lock() = EngineKind::LlamaGguf;
        *self.current_path.lock() = Some(model);
        Ok(())
    }

    /// Anexa a um `llama-server` **explícito** (base URL própria) — segundo
    /// backend (S2/PRO) sem depender de `NOSD_LLAMA_SERVER`.
    pub fn try_attach_llama_at(&self, model: PathBuf, base: String) -> Result<(), SwapError> {
        let backend: Arc<dyn InferenceBackend> =
            Arc::new(LlamaServerBackend::try_new_at(model.clone(), base)?);
        info!(target: "nosd", "hotswap attach(s2) {}", backend.name());
        *self.active.lock() = backend;
        *self.kind.lock() = EngineKind::LlamaGguf;
        *self.current_path.lock() = Some(model);
        Ok(())
    }

    /// Hot-load: unload + attach llama. EXL2/vLLM = use `swap_to` → EngineAbsent.
    pub fn hot_load_llama(&self, model: PathBuf) -> Result<(), SwapError> {
        info!(target: "nosd", "hotswap hot_load {}", model.display());
        self.unload();
        self.try_attach_llama(model)
    }

    /// Protocolo 4 fases (scaffold): freeze→serialize KV→evict→link+warmup.
    pub fn swap_to(&self, kind: EngineKind) -> Result<(), SwapError> {
        info!(target: "nosd", "hotswap freeze/drain kind={:?}", kind);
        info!(target: "nosd", "hotswap kv_serialize (stub session)");
        info!(target: "nosd", "hotswap vram_eviction");
        let backend: Arc<dyn InferenceBackend> = match kind {
            EngineKind::CpuEcho => Arc::new(CpuEchoBackend),
            EngineKind::LlamaGguf => {
                warn!(target: "nosd", "swap_to(LlamaGguf) requires try_attach_llama(path)");
                return Err(SwapError::EngineAbsent("llama.cpp path required"));
            }
            EngineKind::Exl2 | EngineKind::VllmCuda => {
                return Err(SwapError::EngineAbsent("nvidia backend"));
            }
            EngineKind::VllmRocm | EngineKind::LlamaHip => {
                return Err(SwapError::EngineAbsent("amd backend"));
            }
            EngineKind::Ipex | EngineKind::OpenVino | EngineKind::LlamaSycl => {
                return Err(SwapError::EngineAbsent("intel backend"));
            }
        };
        info!(target: "nosd", "hotswap warmup kind={:?}", kind);
        let _ = backend.infer("warmup");
        *self.active.lock() = backend;
        *self.kind.lock() = kind;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_infer() {
        let hs = HotSwapController::new_cpu();
        let out = hs.infer("oi").unwrap();
        assert!(out.contains("oi"));
        assert_eq!(hs.engine_name(), "cpu-echo");
    }

    #[test]
    fn vendor_absent() {
        let hs = HotSwapController::new_cpu();
        assert!(matches!(
            hs.swap_to(EngineKind::Exl2),
            Err(SwapError::EngineAbsent(_))
        ));
    }

    #[test]
    fn attach_missing_model() {
        let hs = HotSwapController::new_cpu();
        let r = hs.try_attach_llama(PathBuf::from("/no/such/model.gguf"));
        assert!(r.is_err());
        assert_eq!(hs.kind(), EngineKind::CpuEcho);
    }

    #[test]
    fn thermal_cap_only_applies_to_explicit_ngl() {
        // Sem `-ngl`, o `--fit` decide: o cap nao inventa um valor.
        assert_eq!(apply_thermal_cap(0, Some(16)), (0, Some(16)));
        assert_eq!(apply_thermal_cap(99, Some(16)), (16, Some(16)));
        assert_eq!(apply_thermal_cap(12, Some(16)), (12, Some(16)));
        assert_eq!(apply_thermal_cap(99, Some(0)), (0, Some(0)));
        assert_eq!(apply_thermal_cap(99, None), (99, None));
    }

    #[test]
    fn thermal_cap_rises_and_falls() {
        set_thermal_ngl_cap(Some(16));
        assert_eq!(thermal_ngl_cap(), Some(16));
        // Arrefeceu: o cap liberta-se (antes ficava colado para sempre).
        set_thermal_ngl_cap(None);
        assert_eq!(thermal_ngl_cap(), None);
    }

    #[test]
    fn load_outcome_reports_substitution() {
        assert!(LoadOutcome::Swapped.swapped());
        assert!(!LoadOutcome::Reused.swapped());
        assert_eq!(LoadOutcome::Reused.as_str(), "reused");
    }

    #[test]
    fn unload_to_echo() {
        let hs = HotSwapController::new_cpu();
        hs.unload();
        assert_eq!(hs.engine_name(), "cpu-echo");
        assert_eq!(hs.kind(), EngineKind::CpuEcho);
    }

    #[test]
    fn concurrent_infer_does_not_hold_lock_across_io() {
        let hs = std::sync::Arc::new(HotSwapController::new_cpu());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let hs = hs.clone();
            handles.push(std::thread::spawn(move || hs.infer("x").unwrap()));
        }
        for h in handles {
            assert!(h.join().unwrap().contains('x'));
        }
    }
}
