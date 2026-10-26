//! Persona viva (P.14) — emotion + heartbeats + VOICE_LATENCY budget.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    Neutral,
    Calm,
    Focused,
    Alert,
    Warm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaState {
    pub emotion: Emotion,
    pub heartbeat_due: bool,
    pub voice_latency_ms_budget: u32,
    pub last_voice_latency_ms: Option<u32>,
}

impl Default for PersonaState {
    fn default() -> Self {
        Self {
            emotion: Emotion::Neutral,
            heartbeat_due: false,
            voice_latency_ms_budget: 1500,
            last_voice_latency_ms: None,
        }
    }
}

impl PersonaState {
    pub fn note_voice_latency(&mut self, ms: u32) {
        self.last_voice_latency_ms = Some(ms);
        if ms > self.voice_latency_ms_budget {
            self.emotion = Emotion::Alert;
        }
    }

    pub fn tick_heartbeat(&mut self, idle_ticks: u64) {
        self.heartbeat_due = idle_ticks > 600;
        if self.heartbeat_due {
            self.emotion = Emotion::Warm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_alerts() {
        let mut p = PersonaState::default();
        p.note_voice_latency(3000);
        assert_eq!(p.emotion, Emotion::Alert);
    }
}
