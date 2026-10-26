//! OrbBroker — estado usabilidade JARBAS + sense (mic/motion) do lab cinemático.
//! Infer Vision MoE = conector (`vision_ready`); não inventa modelo.

use nos_bus::{EventBus, LatentFrame};
use nos_core::orb::topics;
use nos_core::{OrbPattern, OrbSense, OrbSnapshot, OrbState};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct OrbBroker {
    snap: Arc<Mutex<OrbSnapshot>>,
    bus: EventBus,
}

impl OrbBroker {
    pub fn new(bus: EventBus) -> Self {
        Self {
            snap: Arc::new(Mutex::new(OrbSnapshot::default())),
            bus,
        }
    }

    pub fn get(&self) -> OrbSnapshot {
        self.snap.lock().clone()
    }

    pub fn set_state(&self, state: OrbState, apply_presets: bool) -> OrbSnapshot {
        let mut s = self.snap.lock();
        s.state = state;
        if apply_presets {
            s.apply_state_presets();
        }
        let out = s.clone();
        drop(s);
        self.bus.publish(
            topics::ORB_STATE,
            serde_json::to_value(&out).unwrap_or(json!({})),
        );
        info!(
            target: "nosd",
            "orb state={} pattern={} particles={}",
            out.state.as_str(),
            out.pattern.as_str(),
            out.particles
        );
        out
    }

    pub fn set_pattern(&self, pattern: OrbPattern) -> OrbSnapshot {
        let mut s = self.snap.lock();
        s.pattern = pattern;
        let out = s.clone();
        drop(s);
        self.bus.publish(
            topics::ORB_STATE,
            serde_json::to_value(&out).unwrap_or(json!({})),
        );
        out
    }

    /// Patch parcial (lab / nosctl).
    pub fn patch(&self, v: &serde_json::Value) -> OrbSnapshot {
        let mut s = self.snap.lock();
        if let Some(st) = v
            .get("state")
            .and_then(|x| x.as_str())
            .and_then(OrbState::parse)
        {
            s.state = st;
            if v.get("presets").and_then(|x| x.as_bool()).unwrap_or(true) {
                s.apply_state_presets();
            }
        }
        if let Some(p) = v
            .get("pattern")
            .and_then(|x| x.as_str())
            .and_then(OrbPattern::parse)
        {
            s.pattern = p;
        }
        if let Some(x) = v.get("speed").and_then(|x| x.as_f64()) {
            s.speed = (x as f32).clamp(0.2, 3.0);
        }
        if let Some(x) = v.get("size").and_then(|x| x.as_f64()) {
            s.size = (x as f32).clamp(0.4, 2.5);
        }
        if let Some(x) = v.get("particles").and_then(|x| x.as_u64()) {
            s.particles = (x as u32).clamp(16, 800);
        }
        if let Some(x) = v.get("agitation").and_then(|x| x.as_f64()) {
            s.agitation = (x as f32).clamp(0.0, 1.0);
        }
        if let Some(x) = v.get("links").and_then(|x| x.as_f64()) {
            s.links = (x as f32).clamp(0.0, 1.0);
        }
        if let Some(x) = v.get("rings").and_then(|x| x.as_u64()) {
            s.rings = (x as u32).min(8);
        }
        if let Some(b) = v.get("vision_ready").and_then(|x| x.as_bool()) {
            s.vision_ready = b;
        }
        let out = s.clone();
        drop(s);
        self.bus.publish(
            topics::ORB_STATE,
            serde_json::to_value(&out).unwrap_or(json!({})),
        );
        out
    }

    /// LatentBus → intensidade/agitação do orb (P.2).
    pub fn apply_latent(&self, frame: &LatentFrame) -> OrbSnapshot {
        let intensity = frame.v.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let mut s = self.snap.lock();
        s.latent_tag = Some(frame.tag.clone());
        s.sense.level = (s.sense.level * 0.5 + intensity * 0.5).clamp(0.0, 1.0);
        s.agitation = (s.agitation * 0.4 + intensity * 0.6).clamp(0.0, 1.0);
        let out = s.clone();
        drop(s);
        self.bus.publish(
            topics::ORB_SENSE,
            json!({
                "latent_tag": out.latent_tag,
                "level": out.sense.level,
                "agitation": out.agitation,
                "tick": frame.tick,
            }),
        );
        out
    }

    /// Lab → sense; frame_b64 publica ORB_VISION_FRAME (MoE Vision AWAITING).
    pub fn sense(&self, sense: OrbSense) -> OrbSnapshot {
        let has_frame = sense
            .frame_b64
            .as_ref()
            .map(|f| f.len() > 32)
            .unwrap_or(false);
        let mut s = self.snap.lock();
        // Auto-estado a partir do áudio se idle/listening
        if sense.audio_on
            && sense.level > 0.08
            && matches!(s.state, OrbState::Idle | OrbState::Connecting)
        {
            s.state = OrbState::Listening;
            s.apply_state_presets();
        }
        s.sense = sense;
        // Motion aumenta agitação (sem substituir preset)
        s.agitation = (s.agitation * 0.6 + s.sense.motion * 0.4).clamp(0.0, 1.0);
        let out = s.clone();
        drop(s);
        self.bus.publish(
            topics::ORB_SENSE,
            json!({
                "level": out.sense.level,
                "motion": out.sense.motion,
                "video_on": out.sense.video_on,
                "audio_on": out.sense.audio_on,
                "state": out.state.as_str(),
            }),
        );
        if has_frame {
            self.bus.publish(
                topics::ORB_VISION_FRAME,
                json!({
                    "slot": out.vision_slot,
                    "vision_ready": out.vision_ready,
                    "bytes_b64": out.sense.frame_b64.as_ref().map(|f| f.len()).unwrap_or(0),
                    "note": if out.vision_ready {
                        "frame queued — MoE Vision path"
                    } else {
                        "connector_ready_vision_model_awaiting"
                    },
                }),
            );
        }
        out
    }

    /// Fleet HUD fino — publica `FLEET_HEALTH` (JARBAS/lab); não altera lógica de saúde.
    pub fn note_fleet(&self, verdict: &str, peers: u32, machines: u32) -> OrbSnapshot {
        let mut s = self.snap.lock();
        if verdict == "no_go" {
            s.agitation = (s.agitation + 0.25).clamp(0.0, 1.0);
            if matches!(s.state, OrbState::Idle | OrbState::Listening) {
                s.state = OrbState::Alert;
                s.apply_state_presets();
            }
        }
        let out = s.clone();
        drop(s);
        self.bus.publish(
            "FLEET_HEALTH",
            json!({
                "verdict": verdict,
                "peers": peers,
                "machines": machines,
                "orb_state": out.state.as_str(),
                "note": "fleet_worst HUD — CRDT NOT_DONE",
            }),
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sense_lifts_listening() {
        let bus = EventBus::new(8);
        let b = OrbBroker::new(bus);
        let sense = OrbSense {
            audio_on: true,
            level: 0.4,
            ..Default::default()
        };
        let s = b.sense(sense);
        assert_eq!(s.state, OrbState::Listening);
    }
}
