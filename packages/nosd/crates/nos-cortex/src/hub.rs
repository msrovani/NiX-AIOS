//! ModelHub 8 slots + LOO seam (PORT ADR-0078).

use crate::airllm::{plan_airllm, run_layer_stream};
use crate::fit_policy::{fit_model, FitVerdict};
use tracing::info;
use nos_core::ModelSlot;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SlotState {
    pub path: Option<PathBuf>,
    pub loaded: bool,
    pub bytes_len: u64,
}

impl Default for SlotState {
    fn default() -> Self {
        Self {
            path: None,
            loaded: false,
            bytes_len: 0,
        }
    }
}

#[derive(Clone, Default)]
pub struct ModelHub {
    slots: Arc<RwLock<HashMap<ModelSlot, SlotState>>>,
}

impl ModelHub {
    pub fn new() -> Self {
        let mut m = HashMap::new();
        for s in ModelSlot::ALL {
            m.insert(s, SlotState::default());
        }
        Self {
            slots: Arc::new(RwLock::new(m)),
        }
    }

    pub fn register_path(&self, slot: ModelSlot, path: PathBuf, len: u64) -> bool {
        let mut g = self.slots.write();
        let st = g.get_mut(&slot).unwrap();
        st.path = Some(path);
        st.bytes_len = len;
        st.loaded = false;
        true
    }

    /// Estimativa MB do blob no slot (0 se ausente).
    pub fn need_mb(&self, slot: ModelSlot) -> u64 {
        self.slots
            .read()
            .get(&slot)
            .map(|s| s.bytes_len / (1024 * 1024))
            .unwrap_or(0)
            .max(1)
    }

    /// Desmarca o primeiro slot loaded (ordem canônica) — evict coldest stub.
    pub fn evict_coldest(&self) -> Option<ModelSlot> {
        let mut g = self.slots.write();
        for s in ModelSlot::ALL {
            let st = g.get_mut(&s)?;
            if st.loaded {
                st.loaded = false;
                return Some(s);
            }
        }
        None
    }

    /// LOO: marca loaded sob demanda (mmap real fica no HotSwap).
    pub fn ensure_loaded(&self, slot: ModelSlot) -> bool {
        let mut g = self.slots.write();
        let st = g.get_mut(&slot).unwrap();
        if st.path.is_none() {
            return false;
        }
        st.loaded = true;
        true
    }

    /// FitPolicy (T.3): TooTight → evict um loaded, depois LOO.
    pub fn ensure_loaded_fit(
        &self,
        slot: ModelSlot,
        need_mb: u64,
        avail_ram_mb: u64,
        avail_vram_mb: u64,
    ) -> bool {
        let report = fit_model(need_mb, avail_ram_mb, avail_vram_mb);
        if report.suggest_airllm {
            let plan = plan_airllm(need_mb, avail_ram_mb, avail_vram_mb, 32);
            if plan.layer_stream {
                let report_stream = run_layer_stream(&plan, 32, |_| true);
                info!(
                    target: "nosd",
                    "hub airllm stream ok={} passes={} layers_per_pass={} note={}",
                    report_stream.ok,
                    report_stream.passes.len(),
                    plan.layers_per_pass,
                    report_stream.note
                );
            }
        }
        if report.verdict == FitVerdict::TooTight {
            let _ = self.evict_coldest();
        }
        self.ensure_loaded(slot)
    }

    pub fn path_of(&self, slot: ModelSlot) -> Option<PathBuf> {
        self.slots.read().get(&slot).and_then(|s| s.path.clone())
    }

    pub fn loaded_count(&self) -> u8 {
        self.slots
            .read()
            .values()
            .filter(|s| s.loaded)
            .count() as u8
    }

    pub fn list(&self) -> Vec<(ModelSlot, SlotState)> {
        let g = self.slots.read();
        ModelSlot::ALL
            .into_iter()
            .map(|s| (s, g.get(&s).cloned().unwrap_or_default()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loo_marks_loaded() {
        let h = ModelHub::new();
        assert!(!h.ensure_loaded(ModelSlot::Active));
        h.register_path(ModelSlot::Active, PathBuf::from("/tmp/a.gguf"), 100);
        assert!(h.ensure_loaded(ModelSlot::Active));
        assert_eq!(h.loaded_count(), 1);
    }

    #[test]
    fn evict_coldest_clears_loaded() {
        let h = ModelHub::new();
        h.register_path(ModelSlot::Active, PathBuf::from("/tmp/a.gguf"), 100);
        h.register_path(ModelSlot::Vision, PathBuf::from("/tmp/v.gguf"), 100);
        assert!(h.ensure_loaded(ModelSlot::Active));
        assert!(h.ensure_loaded(ModelSlot::Vision));
        assert_eq!(h.loaded_count(), 2);
        let ev = h.evict_coldest().unwrap();
        assert_eq!(h.loaded_count(), 1);
        assert!(h.ensure_loaded(ev));
    }

    #[test]
    fn ensure_loaded_fit_evicts_when_too_tight() {
        let h = ModelHub::new();
        h.register_path(ModelSlot::Active, PathBuf::from("/tmp/a.gguf"), 100);
        h.register_path(ModelSlot::GeneratorPro, PathBuf::from("/tmp/p.gguf"), 100);
        assert!(h.ensure_loaded(ModelSlot::Active));
        assert!(h.ensure_loaded_fit(
            ModelSlot::GeneratorPro,
            8000,
            512,
            512,
        ));
        assert_eq!(h.loaded_count(), 1);
    }
}
