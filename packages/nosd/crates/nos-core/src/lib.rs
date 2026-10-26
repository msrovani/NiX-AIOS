//! Parsers e contratos puros para NixAIOS (std).
//! Evita dual-truth com bare-metal: golden tests cobrem GGUF magic e CapGate bits.

#![forbid(unsafe_code)]

pub mod cap;
pub mod decision;
pub mod gguf;
pub mod hash;
pub mod iteration;
pub mod model_slot;
pub mod orb;
pub mod policy;
pub mod safety_trust;
pub mod score;
pub mod ui_window;

pub use cap::{CapBits, CapGate, CapRegistry, PeerProfile};
pub use decision::{
    s1_backend_effective, s1_backend_from_env, s1_backend_label, s1_blob_present, s1_scorer_absent,
    score_intent_s1, AbstainReason, Decision, S1Backend,
};
pub use gguf::{parse_gguf_header, GgufHeaderInfo};
pub use hash::{ct_eq, fnv1a64, fnv1a64_hex16};
pub use iteration::IterationBudget;
pub use model_slot::ModelSlot;
pub use orb::{OrbPattern, OrbSense, OrbSnapshot, OrbState};
pub use policy::{
    disposition, lab_auto_switch_from_env, policy_matrix_prod, with_abstain, ActDisposition,
    ActDomain,
};
pub use safety_trust::{
    check_invariant, note_violation, trust_allow, violation_count, SafetyInvariant,
    SafetyViolation, TrustToken,
};
pub use score::{NosScore, Sev};
pub use ui_window::{HitlField, UiSource, WindowAction, WindowActionKind, WindowKind, WindowSpec};
