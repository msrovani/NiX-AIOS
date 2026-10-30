//! Parsers e contratos puros para NixAIOS (std).
//! Evita dual-truth com bare-metal: golden tests cobrem GGUF magic e CapGate bits.

#![forbid(unsafe_code)]

pub mod cap;
pub mod cost;
pub mod dag;
pub mod decision;
pub mod gguf;
pub mod hash;
pub mod iteration;
pub mod loop_guard;
pub mod metrics;
pub mod model_slot;
pub mod orb;
pub mod policy;
pub mod proc;
pub mod reported;
pub mod routing;
pub mod safety_trust;
pub mod score;
pub mod trim;
pub mod ui_window;

pub use cap::{CapBits, CapGate, CapRegistry, PeerProfile};
pub use cost::{tokens_estimate, CostModel, JOULES_PER_KWH};
pub use dag::{
    plan as dag_plan, scopes_conflict, DagError, DagNode, DagPlan, MAX_NODES as DAG_MAX_NODES,
};
pub use decision::{
    s1_backend_effective, s1_backend_from_env, s1_backend_label, s1_blob_present, s1_scorer_absent,
    score_intent_s1, AbstainReason, Decision, S1Backend,
};
pub use gguf::{parse_gguf_header, GgufHeaderInfo};
pub use hash::{ct_eq, fnv1a64, fnv1a64_hex16};
pub use iteration::IterationBudget;
pub use loop_guard::LoopGuard;
pub use metrics::{ScoreWeights, DEFAULT_WEIGHTS};
pub use model_slot::ModelSlot;
pub use orb::{OrbPattern, OrbSense, OrbSnapshot, OrbState};
pub use policy::{
    disposition, lab_auto_switch_from_env, policy_matrix_prod, with_abstain, ActDisposition,
    ActDomain,
};
pub use proc::{first_existing, nvidia_smi_candidates, nvidia_smi_path, NVIDIA_SMI_NIXOS};
pub use reported::{reported_rate, ReportedRate, BASIS_NONE, BASIS_OBSERVED};
pub use routing::{plan_backend, Backend, Lane, RouteInputs, RoutePlan};
pub use safety_trust::{
    check_invariant, note_violation, trust_allow, violation_count, SafetyInvariant,
    SafetyViolation, TrustToken,
};
pub use score::{NosScore, Sev};
pub use trim::{strip_ansi, trim_diagnostic, MAX_DIAG_CHARS};
pub use ui_window::{
    tier_for_grant_kind, HitlField, HitlTier, UiSource, WindowAction, WindowActionKind, WindowKind,
    WindowSpec,
};
