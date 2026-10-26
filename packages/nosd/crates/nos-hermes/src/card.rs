//! J.2 — Generative Card thin: `CardSpec` JSON → janela Custom/Info.

use crate::{HermesEngine, HermesError};
use nos_core::{CapGate, UiSource, WindowKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardSpec {
    pub title: String,
    #[serde(default)]
    pub kv: Vec<CardKv>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardKv {
    pub key: String,
    pub value: String,
}

/// `card: {"title":"...", "kv":[{"key":"a","value":"b"}]}` ou `card: Título|k=v|k2=v2`.
pub fn parse_card_prompt(prompt: &str) -> Option<CardSpec> {
    let rest = prompt.trim().strip_prefix("card:")?.trim();
    if rest.starts_with('{') {
        return serde_json::from_str::<CardSpec>(rest).ok();
    }
    let mut parts = rest.split('|');
    let title = parts.next()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let mut kv = Vec::new();
    for p in parts {
        let p = p.trim();
        if let Some((k, v)) = p.split_once('=') {
            kv.push(CardKv {
                key: k.trim().to_string(),
                value: v.trim().to_string(),
            });
        }
    }
    Some(CardSpec { title, kv })
}

pub fn card_body_from_spec(spec: &CardSpec) -> String {
    let mut lines = vec![spec.title.clone()];
    for pair in &spec.kv {
        lines.push(format!("{}: {}", pair.key, pair.value));
    }
    lines.join("\n")
}

impl HermesEngine {
    pub fn open_generative_card(
        &self,
        gate: &CapGate,
        cap_gen: u64,
        spec: &CardSpec,
        source: UiSource,
    ) -> Result<Value, HermesError> {
        // Display thin (ADR-0058): sempre Info — kv vai no body; sem HITL escalate.
        let body = card_body_from_spec(spec);
        let w = self
            .windows
            .open_from_kind(
                gate,
                cap_gen,
                WindowKind::Info,
                source,
                Some(spec.title.clone()),
                Some(body),
            )
            .map_err(HermesError::Ui)?;
        Ok(serde_json::to_value(w).unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_card() {
        let s = parse_card_prompt(r#"card: {"title":"Clima","kv":[{"key":"temp","value":"22C"}]}"#)
            .unwrap();
        assert_eq!(s.title, "Clima");
        assert_eq!(s.kv.len(), 1);
    }

    #[test]
    fn parse_pipe_card() {
        let s = parse_card_prompt("card: Sistema|cpu=42%|ram=8G").unwrap();
        assert_eq!(s.title, "Sistema");
        assert_eq!(s.kv[0].value, "42%");
    }

    #[test]
    fn non_card_none() {
        assert!(parse_card_prompt("oi").is_none());
    }
}
