//! ModelHub 8 slots + LOO seam (PORT ADR-0078).

use crate::airllm::{plan_airllm, run_layer_stream};
use crate::fit_policy::{fit_model, FitVerdict};
use nos_core::ModelSlot;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct SlotState {
    pub path: Option<PathBuf>,
    pub loaded: bool,
    pub bytes_len: u64,
    /// Todos os GGUF que cairam neste slot (o boot regista tudo o que encontra).
    pub candidates: Vec<PathBuf>,
    /// Mais de um path distinto para o mesmo slot: o hub **nao** escolhe sozinho.
    pub conflict: bool,
}

/// C2.2: necessidade estimada de um slot — ficheiro + KV cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeedEstimate {
    pub slot: String,
    pub file_mb: u64,
    pub kv_mb: u64,
    pub need_vram_mb: u64,
    pub need_ram_mb: u64,
    pub ctx: u32,
    pub layers: u32,
    /// `estimate` = ficheiro + KV por formula (nunca "measured" sem sonda).
    pub source: String,
}

/// KV cache estimado: 2 tensores (K,V) x camadas x ctx x 128 (head_dim) x 2 B.
/// E **estimativa**: o `block_count` real vive na seccao KV do GGUF, que
/// `parse_gguf_header` nao le — por isso o campo `source` existe.
pub fn kv_est_mb(ctx: u32, layers: u32) -> u64 {
    let bytes = 2u64 * layers as u64 * ctx as u64 * 128 * 2;
    bytes.div_ceil(1024 * 1024)
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

    /// C2.5: last-write-wins deixava o 9B herdar o `GeneratorPro` do 4B.
    /// O primeiro path manda; os outros ficam em `candidates` com
    /// `conflict=true` para o RPC poder mostrar o conflito em vez de o esconder.
    pub fn register_path(&self, slot: ModelSlot, path: PathBuf, len: u64) -> bool {
        let mut g = self.slots.write();
        let st = g.get_mut(&slot).unwrap();
        if st.path.as_ref() == Some(&path) {
            st.bytes_len = len;
            return true;
        }
        if let Some(existing) = st.path.clone() {
            if !st.candidates.contains(&path) {
                st.candidates.push(path.clone());
            }
            st.conflict = true;
            info!(
                target: "nosd",
                "hub slot={} conflict keep={} ignored={}",
                slot.name(),
                existing.display(),
                path.display()
            );
            return false;
        }
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

    /// C2.2: `need_mb` era so o tamanho do ficheiro — o KV ficava de fora e o
    /// fit aprovava modelos que depois nao cabiam com contexto.
    pub fn need_estimate(&self, slot: ModelSlot) -> NeedEstimate {
        let file_mb = self.need_mb(slot);
        let ctx = std::env::var("NOSD_LLAMA_FIT_CTX")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| *n >= 512)
            .unwrap_or(2048);
        let layers = std::env::var("NOSD_LLAMA_FIT_LAYERS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(32);
        let kv_mb = kv_est_mb(ctx, layers);
        NeedEstimate {
            slot: slot.name().to_string(),
            file_mb,
            kv_mb,
            // Pesos na VRAM; KV + overhead de runtime na RAM.
            need_vram_mb: file_mb,
            need_ram_mb: kv_mb.saturating_add(256),
            ctx,
            layers,
            source: "estimate".into(),
        }
    }

    /// C2.3: `evict_coldest` so marcava `loaded=false` — a VRAM continuava
    /// ocupada. Este descarrega o motor (-> cpu-echo) **e** desmarca o slot.
    pub fn evict(&self, hotswap: &crate::hotswap::HotSwapController) -> Option<ModelSlot> {
        let slot = self.evict_coldest()?;
        hotswap.unload();
        info!(
            target: "nosd",
            "hub evict slot={} engine={}",
            slot.name(),
            hotswap.engine_name()
        );
        Some(slot)
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
        need_vram_mb: u64,
        need_ram_mb: u64,
        avail_ram_mb: u64,
        avail_vram_mb: u64,
        allow_spill: bool,
    ) -> bool {
        let report = fit_model(
            need_vram_mb,
            need_ram_mb,
            avail_ram_mb,
            avail_vram_mb,
            allow_spill,
        );
        if report.suggest_airllm {
            let plan = plan_airllm(
                need_vram_mb.saturating_add(need_ram_mb),
                avail_ram_mb,
                avail_vram_mb,
                32,
            );
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
            info!(
                target: "nosd",
                "hub slot={} TooTight basis={:?} need_vram={}MB need_ram={}MB vram_free={}MB ram_avail={}MB spill={}",
                slot.name(),
                report.basis,
                need_vram_mb,
                need_ram_mb,
                avail_vram_mb,
                avail_ram_mb,
                allow_spill
            );
            let _ = self.evict_coldest();
        }
        self.ensure_loaded(slot)
    }

    pub fn path_of(&self, slot: ModelSlot) -> Option<PathBuf> {
        self.slots.read().get(&slot).and_then(|s| s.path.clone())
    }

    pub fn loaded_count(&self) -> u8 {
        self.slots.read().values().filter(|s| s.loaded).count() as u8
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
    fn kv_estimate_is_not_zero_and_scales_with_ctx() {
        assert!(kv_est_mb(2048, 32) > 0);
        assert!(kv_est_mb(4096, 32) > kv_est_mb(2048, 32));
    }

    #[test]
    fn need_estimate_says_it_is_an_estimate() {
        let h = ModelHub::new();
        h.register_path(
            ModelSlot::GeneratorPro,
            PathBuf::from("/tmp/p.gguf"),
            5_500_000_000,
        );
        let e = h.need_estimate(ModelSlot::GeneratorPro);
        assert_eq!(e.source, "estimate");
        assert!(e.need_ram_mb > 0, "KV/overhead nao pode ser zero");
        assert!(e.need_vram_mb >= 5000);
    }

    #[test]
    fn slot_conflict_is_reported_not_overwritten() {
        let h = ModelHub::new();
        assert!(h.register_path(ModelSlot::GeneratorPro, PathBuf::from("/tmp/a.gguf"), 10));
        assert!(!h.register_path(ModelSlot::GeneratorPro, PathBuf::from("/tmp/b.gguf"), 20));
        assert_eq!(
            h.path_of(ModelSlot::GeneratorPro),
            Some(PathBuf::from("/tmp/a.gguf"))
        );
        let st = h
            .list()
            .into_iter()
            .find(|(s, _)| *s == ModelSlot::GeneratorPro)
            .expect("slot listado")
            .1;
        assert!(st.conflict);
        assert_eq!(st.candidates.len(), 1);
    }

    #[test]
    fn evict_unloads_the_engine() {
        let h = ModelHub::new();
        let hs = crate::hotswap::HotSwapController::new_cpu();
        h.register_path(ModelSlot::Active, PathBuf::from("/tmp/a.gguf"), 100);
        assert!(h.ensure_loaded(ModelSlot::Active));
        assert!(h.evict(&hs).is_some());
        assert_eq!(h.loaded_count(), 0);
        assert_eq!(hs.engine_name(), "cpu-echo");
    }

    #[test]
    fn ensure_loaded_fit_evicts_when_too_tight() {
        let h = ModelHub::new();
        h.register_path(ModelSlot::Active, PathBuf::from("/tmp/a.gguf"), 100);
        h.register_path(ModelSlot::GeneratorPro, PathBuf::from("/tmp/p.gguf"), 100);
        assert!(h.ensure_loaded(ModelSlot::Active));
        assert!(h.ensure_loaded_fit(ModelSlot::GeneratorPro, 8000, 256, 512, 512, true));
        assert_eq!(h.loaded_count(), 1);
    }
}
