//! Parsers e contratos puros para NixAIOS (std).
//! Evita dual-truth com bare-metal: golden tests cobrem GGUF magic e CapGate bits.

#![forbid(unsafe_code)]

pub mod cap;
pub mod decision;
pub mod hash;
pub mod iteration;
pub mod safety_trust;
pub mod gguf;
pub mod model_slot;
pub mod orb;
pub mod policy;
pub mod score;
pub mod ui_window;

pub use cap::{CapBits, CapGate, CapRegistry, PeerProfile};
pub use decision::{
    AbstainReason, Decision, S1Backend, s1_backend_effective, s1_backend_from_env, s1_backend_label,
    s1_blob_present, score_intent_s1,
};
pub use iteration::IterationBudget;
pub use safety_trust::{
    SafetyInvariant, SafetyViolation, TrustToken, check_invariant, note_violation, trust_allow,
    violation_count,
};
pub use gguf::{GgufHeaderInfo, parse_gguf_header};
pub use hash::{fnv1a64, fnv1a64_hex16};
pub use model_slot::ModelSlot;
pub use orb::{OrbPattern, OrbSense, OrbSnapshot, OrbState};
pub use policy::{
    ActDisposition, ActDomain, disposition, lab_auto_switch_from_env, policy_matrix_prod,
    with_abstain,
};
pub use score::{NosScore, Sev};
pub use ui_window::{
    HitlField, UiSource, WindowAction, WindowActionKind, WindowKind, WindowSpec,
};
