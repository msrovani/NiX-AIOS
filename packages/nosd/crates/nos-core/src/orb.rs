//! Orb JARBAS — contrato VoiceOrbs-like + padrões iron (cinemático).
//! Caminho quente = params baratos; lab vídeo/som alimenta `OrbSense`.

use serde::{Deserialize, Serialize};

/// Ciclo de usabilidade (espelha VoiceOrbs + soul_mirror bare-metal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrbState {
    #[default]
    Idle,
    Connecting,
    Listening,
    Thinking,
    Speaking,
    Dreaming,
    Degraded,
    Alert,
    Error,
    Disabled,
}

impl OrbState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Connecting => "connecting",
            Self::Listening => "listening",
            Self::Thinking => "thinking",
            Self::Speaking => "speaking",
            Self::Dreaming => "dreaming",
            Self::Degraded => "degraded",
            Self::Alert => "alert",
            Self::Error => "error",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "idle" => Some(Self::Idle),
            "connecting" | "connect" => Some(Self::Connecting),
            "listening" | "listen" => Some(Self::Listening),
            "thinking" | "think" => Some(Self::Thinking),
            "speaking" | "speak" => Some(Self::Speaking),
            "dreaming" | "dream" => Some(Self::Dreaming),
            "degraded" => Some(Self::Degraded),
            "alert" => Some(Self::Alert),
            "error" => Some(Self::Error),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }

    /// Cor acento RGB sugerida (HUD / lab).
    pub fn accent_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Idle => (0, 212, 255),
            Self::Connecting => (100, 180, 255),
            Self::Listening => (0, 255, 255),
            Self::Thinking => (255, 148, 48),
            Self::Speaking => (0, 230, 255),
            Self::Dreaming => (130, 100, 235),
            Self::Degraded => (255, 176, 32),
            Self::Alert | Self::Error => (255, 48, 56),
            Self::Disabled => (80, 90, 100),
        }
    }
}

/// Padrões geométricos estilo iron-interface (morph barato no lab; preset no HUD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrbPattern {
    #[default]
    Sphere,
    Ring,
    Helix,
    Spiral,
    Galaxy,
    Wave,
    Cluster,
}

impl OrbPattern {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sphere => "sphere",
            Self::Ring => "ring",
            Self::Helix => "helix",
            Self::Spiral => "spiral",
            Self::Galaxy => "galaxy",
            Self::Wave => "wave",
            Self::Cluster => "cluster",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sphere" | "esfera" => Some(Self::Sphere),
            "ring" | "anel" | "donut" => Some(Self::Ring),
            "helix" | "helice" | "hélice" => Some(Self::Helix),
            "spiral" | "espiral" => Some(Self::Spiral),
            "galaxy" | "galaxia" | "galáxia" => Some(Self::Galaxy),
            "wave" | "onda" => Some(Self::Wave),
            "cluster" | "nuvem" => Some(Self::Cluster),
            _ => None,
        }
    }

    pub const ALL: [OrbPattern; 7] = [
        Self::Sphere,
        Self::Ring,
        Self::Helix,
        Self::Spiral,
        Self::Galaxy,
        Self::Wave,
        Self::Cluster,
    ];
}

/// Captura do lab (mic + movimento) → JARBAS / Hermes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrbSense {
    /// Amplitude 0..1 (AnalyserNode / RMS).
    #[serde(default)]
    pub level: f32,
    #[serde(default)]
    pub bass: f32,
    #[serde(default)]
    pub mid: f32,
    #[serde(default)]
    pub treble: f32,
    /// Agitação óptica 0..1 (frame-diff webcam; sem modelo ainda).
    #[serde(default)]
    pub motion: f32,
    /// Webcam ativa no lab.
    #[serde(default)]
    pub video_on: bool,
    /// Mic activo.
    #[serde(default)]
    pub audio_on: bool,
    /// Frame JPEG/PNG base64 opcional — Vision MoE (AWAITING infer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_b64: Option<String>,
}

/// Snapshot completo do orb (HUD + lab + EventBus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbSnapshot {
    pub state: OrbState,
    pub pattern: OrbPattern,
    /// Multiplicador de animação (VoiceOrbs `speed`).
    pub speed: f32,
    /// Escala visual 0.5..2.0.
    pub size: f32,
    /// Contagem alvo de partículas (HUD soft-cap).
    pub particles: u32,
    /// Agitação procedural extra 0..1.
    pub agitation: f32,
    /// Densidade de ligações entre partículas 0..1.
    pub links: f32,
    /// Anéis orbitais 0..8.
    pub rings: u32,
    pub sense: OrbSense,
    /// Slot MoE sugerido p/ frame (Vision) — conector; infer = AWAITING.
    #[serde(default)]
    pub vision_slot: String,
    #[serde(default)]
    pub vision_ready: bool,
    /// Tag do último LatentFrame aplicado (P.2 Orb).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latent_tag: Option<String>,
}

impl Default for OrbSnapshot {
    fn default() -> Self {
        Self {
            state: OrbState::Idle,
            pattern: OrbPattern::Sphere,
            speed: 1.0,
            size: 1.0,
            particles: 120,
            agitation: 0.15,
            links: 0.2,
            rings: 2,
            sense: OrbSense::default(),
            vision_slot: "vision".into(),
            vision_ready: false,
            latent_tag: None,
        }
    }
}

impl OrbSnapshot {
    /// Ajusta presets baratos a partir do estado (VoiceOrbs contract).
    pub fn apply_state_presets(&mut self) {
        match self.state {
            OrbState::Idle => {
                self.speed = 0.7;
                self.agitation = 0.1;
                self.particles = 80;
                self.rings = 2;
                self.links = 0.1;
            }
            OrbState::Listening => {
                self.speed = 1.0;
                self.agitation = 0.35;
                self.particles = 140;
                self.rings = 3;
                self.links = 0.35;
                self.pattern = OrbPattern::Ring;
            }
            OrbState::Thinking => {
                self.speed = 1.4;
                self.agitation = 0.55;
                self.particles = 200;
                self.rings = 4;
                self.links = 0.6;
                self.pattern = OrbPattern::Helix;
            }
            OrbState::Speaking => {
                self.speed = 1.2;
                self.agitation = 0.45;
                self.particles = 160;
                self.rings = 3;
                self.links = 0.4;
                self.pattern = OrbPattern::Wave;
            }
            OrbState::Alert | OrbState::Error => {
                self.speed = 1.8;
                self.agitation = 0.85;
                self.particles = 220;
                self.rings = 5;
                self.links = 0.15;
            }
            OrbState::Dreaming => {
                self.speed = 0.5;
                self.agitation = 0.2;
                self.particles = 100;
                self.pattern = OrbPattern::Galaxy;
            }
            _ => {}
        }
    }
}

/// Tópicos EventBus.
pub mod topics {
    pub const ORB_STATE: &str = "ORB_STATE";
    pub const ORB_SENSE: &str = "ORB_SENSE";
    pub const ORB_VISION_FRAME: &str = "ORB_VISION_FRAME";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        assert_eq!(OrbState::parse("listening"), Some(OrbState::Listening));
        assert_eq!(OrbPattern::parse("helix"), Some(OrbPattern::Helix));
    }

    #[test]
    fn listen_preset() {
        let mut s = OrbSnapshot {
            state: OrbState::Listening,
            ..Default::default()
        };
        s.apply_state_presets();
        assert!(s.rings >= 3);
        assert_eq!(s.pattern, OrbPattern::Ring);
    }
}
