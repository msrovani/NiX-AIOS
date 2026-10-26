//! Cortex std: ModelHub, scanner, capacity, HotSwap, distrib assign.

#![forbid(unsafe_code)]

pub mod bge_embed;
pub mod capacity;
pub mod distrib;
pub mod fetch;
pub mod airllm;
pub mod fit_policy;
pub mod header_scan;
pub mod hotswap;
pub mod hub;
pub mod offload;
pub mod route;
pub mod router_score;
pub mod s2_queue;
pub mod slot_map;
pub mod ui_hint;
pub mod vision_rerank;

pub use airllm::{plan_airllm, run_layer_stream, AirLlmPass, AirLlmPlan, AirLlmStreamReport};
pub use fit_policy::{fit_model, FitReport, FitVerdict};

pub use capacity::{CapacityProber, NodeCapacity};
pub use distrib::{assign_from_mesh_tuples, capacity_weighted_assign, PeerCapacity};
pub use fetch::{
    FetchError, HfModelHit, ModelPlan, QuantSuggest, fetch_hf_gguf, fetch_to_dir, hf_resolve_url,
    plan_models_for_capacity, search_hf_gguf, suggest_for_capacity,
};
pub use header_scan::{ModelMeta, scan_path};
pub use hotswap::{
    EngineKind, HotSwapController, InferenceBackend, LlamaCliBackend, SwapError, find_llama_cli,
    llama_server_base, LlamaServerBackend,
};
pub use hub::ModelHub;
pub use offload::plan_n_gpu_layers;
pub use route::{RouteDecision, SystemLane, route_intent, route_intent_keyword};
pub use router_score::{RouteScores, score_intent};
pub use s2_queue::{S2Queue, S2Result};
pub use slot_map::slot_from_path;
pub use bge_embed::{BgeStatus, bge_status, embed_lexical_fallback};
pub use ui_hint::suggest_window_kind;
pub use vision_rerank::{
    VisionRerankResolve, VisionRerankRole, resolve_vision_rerank, role_from_prompt,
};
