//! Contrato de janelas JARBAS — objetos de cerimónia (mostrar / HITL).
//! Cadeia: Cortex sugere → Hermes orquestra → JARBAS materializa → acção → Hermes.

use serde::{Deserialize, Serialize};

/// Quem pediu a janela (provenance no broker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSource {
    Jarbas,
    Hermes,
    Cortex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// Só informação (sem campos).
    Info,
    /// Status de sistema / motor.
    Status,
    /// HITL: lista SSIDs + senha.
    HitlWifi,
    /// HITL: confirmar/deny (nix.propose, CapGate escalate).
    HitlConfirm,
    /// HITL: texto livre.
    HitlText,
    /// Extensível (JSON fields).
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlField {
    pub name: String,
    pub label: String,
    #[serde(default = "field_text")]
    pub kind: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub required: bool,
}

fn field_text() -> String {
    "text".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSpec {
    pub id: u64,
    pub kind: WindowKind,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub fields: Vec<HitlField>,
    pub source: UiSource,
    /// CapGate Escalate — JARBAS deve pedir confirmação explícita.
    #[serde(default)]
    pub escalate: bool,
    #[serde(default)]
    pub topic_reply: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowActionKind {
    Submit,
    Cancel,
    Dismiss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowAction {
    pub window_id: u64,
    pub action: WindowActionKind,
    #[serde(default)]
    pub values: serde_json::Map<String, serde_json::Value>,
}

/// Tópicos EventBus canónicos (conectores).
pub mod topics {
    pub const UI_OPEN: &str = "UI_WINDOW_OPEN";
    pub const UI_CLOSE: &str = "UI_WINDOW_CLOSE";
    pub const UI_ACTION: &str = "UI_WINDOW_ACTION";
    pub const UI_HITL_RESULT: &str = "UI_HITL_RESULT";
}

impl WindowSpec {
    pub fn info(
        id: u64,
        title: impl Into<String>,
        body: impl Into<String>,
        source: UiSource,
    ) -> Self {
        Self {
            id,
            kind: WindowKind::Info,
            title: title.into(),
            body: body.into(),
            fields: vec![],
            source,
            escalate: false,
            topic_reply: topics::UI_HITL_RESULT.into(),
        }
    }

    pub fn hitl_wifi(id: u64, ssids: Vec<String>, source: UiSource) -> Self {
        Self {
            id,
            kind: WindowKind::HitlWifi,
            title: "Wi‑Fi".into(),
            body: "Seleccione a rede e introduza a senha (HITL JARBAS).".into(),
            fields: vec![
                HitlField {
                    name: "ssid".into(),
                    label: "Rede".into(),
                    kind: "select".into(),
                    options: ssids,
                    secret: false,
                    required: true,
                },
                HitlField {
                    name: "password".into(),
                    label: "Senha".into(),
                    kind: "password".into(),
                    options: vec![],
                    secret: true,
                    required: true,
                },
            ],
            source,
            escalate: true,
            topic_reply: topics::UI_HITL_RESULT.into(),
        }
    }

    pub fn hitl_confirm(
        id: u64,
        title: impl Into<String>,
        body: impl Into<String>,
        source: UiSource,
    ) -> Self {
        Self {
            id,
            kind: WindowKind::HitlConfirm,
            title: title.into(),
            body: body.into(),
            fields: vec![],
            source,
            escalate: true,
            topic_reply: topics::UI_HITL_RESULT.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_fields() {
        let w = WindowSpec::hitl_wifi(1, vec!["lab".into()], UiSource::Hermes);
        assert_eq!(w.kind, WindowKind::HitlWifi);
        assert_eq!(w.fields.len(), 2);
        assert!(w.escalate);
    }
}
