//! Blobs/firmware externos — só runtime, deliberados Cortex→Hermes→JARBAS.
//! Anti-padrão: embutir modelos/firmware na ISO (S1.2 seed REJECT).

use nos_bei::PersonaState;
use nos_core::{
    disposition, lab_auto_switch_from_env, with_abstain, ActDisposition, ActDomain, Decision,
    AbstainReason, CapBits, CapGate,
};
use nos_cortex::{fit_model, FitVerdict, NodeCapacity, suggest_for_capacity};
use nos_memory::{MemoryRecord, NosMemory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobKind {
    ModelGguf,
    ModelBitnet,
    VoiceWhisper,
    VoicePiper,
    Firmware,
    WasmSkill,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobNeed {
    pub kind: BlobKind,
    pub goal: String,
    /// URL ou repo/file HF — ainda não descarregado.
    pub source: String,
    pub estimated_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobDeliberation {
    pub need: BlobNeed,
    pub cortex_plan: String,
    pub quant_hint: Option<String>,
    pub fit: FitVerdict,
    pub disposition: ActDisposition,
    pub hitl_required: bool,
    pub allow_fetch: bool,
    pub reason: String,
    pub decision: Decision<bool>,
}

/// Delibera download: Cortex (capacity/fit) → Hermes (policy) → JARBAS HITL se Confirm.
/// `hitl_confirmed` = utilizador aprovou card JARBAS.
pub fn deliberate_blob_fetch(
    need: &BlobNeed,
    cap: &NodeCapacity,
    hitl_confirmed: bool,
    scorer_absent: bool,
) -> BlobDeliberation {
    let suggest = suggest_for_capacity(cap);
    let fit_rep = fit_model(
        need.estimated_mb.max(1),
        cap.ram_avail_mb.max(1),
        cap.vram_free_mb,
    );
    let cortex_plan = format!(
        "kind={:?} src={} est_mb={} quant={} ngl={} fit={:?} airllm={}",
        need.kind,
        need.source,
        need.estimated_mb,
        suggest.quant,
        suggest.suggest_ngl,
        fit_rep.verdict,
        fit_rep.suggest_airllm
    );

    let domain = match need.kind {
        BlobKind::Firmware => ActDomain::DeviceFirmware,
        BlobKind::WasmSkill => ActDomain::FactorySeekInstall,
        _ => ActDomain::ModelFetch,
    };
    let mut disp = disposition(domain, lab_auto_switch_from_env());
    // Firmware sempre Escalate/Confirm — nunca Auto silencioso.
    if matches!(need.kind, BlobKind::Firmware) {
        disp = ActDisposition::Escalate;
    }
    // Downloads grandes (>400MB) forçam Confirm mesmo se Auto.
    if need.estimated_mb >= 400 && disp == ActDisposition::Auto {
        disp = ActDisposition::Confirm;
    }
    disp = with_abstain(disp, scorer_absent);

    let hitl_required = matches!(
        disp,
        ActDisposition::Confirm | ActDisposition::Escalate
    );
    let denied = disp == ActDisposition::Deny;
    let allow_fetch = !denied && (!hitl_required || hitl_confirmed);

    let decision = if denied {
        Decision::abstain(AbstainReason::HitlRequired, "blob_deny")
    } else if hitl_required && !hitl_confirmed {
        Decision::abstain(AbstainReason::HitlRequired, "blob_await_jarbas")
    } else if scorer_absent && matches!(need.kind, BlobKind::Firmware) {
        Decision::abstain(AbstainReason::ScorerAbsent, "blob_firmware")
    } else {
        Decision::decide(true, 180, 40, "blob_deliberate")
    };

    let reason = if allow_fetch {
        "Cortex plan OK; Hermes allow; JARBAS confirmed or Auto".into()
    } else if denied {
        "SITE_POLICY deny blob fetch".into()
    } else {
        format!("await JARBAS HITL disposition={disp:?}")
    };

    BlobDeliberation {
        need: need.clone(),
        cortex_plan,
        quant_hint: Some(suggest.quant),
        fit: fit_rep.verdict,
        disposition: disp,
        hitl_required,
        allow_fetch,
        reason,
        decision,
    }
}

/// Persiste deliberação (Remember) — auditoria ADR-0113.
pub fn remember_deliberation(memory: &NosMemory, d: &BlobDeliberation) -> bool {
    let key = format!("md/L3/blob/{}", Uuid::new_v4());
    let text = serde_json::to_string(d).unwrap_or_else(|_| d.reason.clone());
    memory
        .remember(MemoryRecord {
            key,
            text,
            scope: "session/blob_fetch".into(),
            entities: vec![
                "mom/decision".into(),
                "blob/runtime_fetch".into(),
                format!("kind/{:?}", d.need.kind),
            ],
        })
        .is_ok()
}

/// CapGate: fetch exige NET|FS; firmware também pede HITL bit se Confirm.
pub fn gate_allows_fetch(gate: &CapGate, gen: u64, need_hitl: bool) -> Result<(), &'static str> {
    gate.allow(CapBits::NET.union(CapBits::FS), gen)?;
    if need_hitl {
        gate.allow(CapBits::HITL, gen)?;
    }
    Ok(())
}

/// Política ISO: nunca embutir blobs grandes no image — só runtime.
pub fn iso_must_not_ship_blobs() -> &'static str {
    "REJECT: external models/firmware/voice blobs are runtime-fetched after Cortex/Hermes/JARBAS deliberation — not ISO-seeded"
}

/// Persona hint quando download longo.
pub fn note_persona_download(persona: &mut PersonaState, est_mb: u64) {
    if est_mb >= 200 {
        persona.emotion = nos_bei::Emotion::Focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nos_cortex::capacity::NodeCapacity;

    fn cap(ram: u64, vram: u64) -> NodeCapacity {
        NodeCapacity {
            ram_total_mb: ram + 1024,
            ram_avail_mb: ram,
            vram_free_mb: vram,
            dri_cards: vec![],
            dri_render: vec![],
            vendor_hint: "nvidia".into(),
            accel: vec![],
        }
    }

    #[test]
    fn model_fetch_needs_jarbas() {
        // SITE_POLICY ModelFetch = Confirm → sem hitl_confirmed não descarrega.
        let need = BlobNeed {
            kind: BlobKind::ModelGguf,
            goal: "tiny".into(),
            source: "https://example/x.gguf".into(),
            estimated_mb: 50,
        };
        let d = deliberate_blob_fetch(&need, &cap(8000, 8000), false, false);
        assert!(d.hitl_required);
        assert!(!d.allow_fetch);
        let d2 = deliberate_blob_fetch(&need, &cap(8000, 8000), true, false);
        assert!(d2.allow_fetch);
    }

    #[test]
    fn large_requires_hitl() {
        let need = BlobNeed {
            kind: BlobKind::ModelGguf,
            goal: "pro".into(),
            source: "hf/x".into(),
            estimated_mb: 900,
        };
        let d = deliberate_blob_fetch(&need, &cap(16000, 8000), false, false);
        assert!(d.hitl_required);
        assert!(!d.allow_fetch);
        let d2 = deliberate_blob_fetch(&need, &cap(16000, 8000), true, false);
        assert!(d2.allow_fetch);
    }

    #[test]
    fn firmware_never_auto() {
        let need = BlobNeed {
            kind: BlobKind::Firmware,
            goal: "iwlwifi".into(),
            source: "https://fw/x".into(),
            estimated_mb: 5,
        };
        let d = deliberate_blob_fetch(&need, &cap(8000, 0), false, false);
        assert!(d.hitl_required);
        assert!(!d.allow_fetch);
    }

    #[test]
    fn iso_policy_message() {
        assert!(iso_must_not_ship_blobs().contains("runtime"));
    }
}
