//! Cortex std: ModelHub, scanner, capacity, HotSwap, distrib assign.

#![forbid(unsafe_code)]

pub mod airllm;
pub mod bge_embed;
pub mod capacity;
pub mod distrib;
pub mod fetch;
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

pub use bge_embed::{bge_status, embed_lexical_fallback, BgeStatus};
pub use capacity::{CapacityProber, NodeCapacity};
pub use distrib::{assign_from_mesh_tuples, capacity_weighted_assign, PeerCapacity};
pub use fetch::{
    fetch_hf_gguf, fetch_to_dir, hf_resolve_url, plan_models_for_capacity, search_hf_gguf,
    suggest_for_capacity, FetchError, HfModelHit, ModelPlan, QuantSuggest,
};
pub use header_scan::{scan_path, ModelMeta};
pub use hotswap::{
    find_llama_cli, llama_server_base, llama_server_posture, EngineKind, HotSwapController,
    InferenceBackend, LlamaCliBackend, LlamaServerBackend, SwapError,
};
pub use hub::ModelHub;
pub use offload::plan_n_gpu_layers;
pub use route::{route_intent, route_intent_keyword, RouteDecision, SystemLane};
pub use router_score::{score_intent, RouteScores};
pub use s2_queue::{S2Queue, S2Result};
pub use slot_map::slot_from_path;
pub use ui_hint::suggest_window_kind;
pub use vision_rerank::{
    resolve_vision_rerank, role_from_prompt, VisionRerankResolve, VisionRerankRole,
};
