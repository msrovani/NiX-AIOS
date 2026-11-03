//! RPC — domínio Nix (leitura pura).
//!
//! `nix.status`: factos vivos da máquina (versão, geração, flake, ring).
//! `nix.ask`: pergunta → lições operacionais que custaram builds/sessões.
//!
//! Estes métodos são o mesmo conhecimento que o córtex/hermes injectam no
//! prompt (`## Nix (live)`), expostos para a CLI e para o HUD — uma só fonte.

use super::*;
use nos_core::nix_domain;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(_state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "nix.status" => {
            let facts = nix_domain::probe();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "facts": facts,
                    "line": facts.line(),
                    "lessons": nix_domain::LESSONS.len(),
                    "honesty": "lido do sistema; ausente = null",
                }),
            }
        }
        "nix.ask" => {
            let q = req
                .params
                .get("question")
                .or_else(|| req.params.get("q"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if q.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("params.question required".into()),
                    result: json!(null),
                });
            }
            let hits = nix_domain::lookup(&q, 4);
            let lessons: Vec<serde_json::Value> = hits
                .iter()
                .map(|l| json!({ "id": l.id, "topic": l.topic, "text": l.text }))
                .collect();
            let facts = nix_domain::probe();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "question": q,
                    "match": lessons.len(),
                    "lessons": lessons,
                    "facts": facts,
                    "line": facts.line(),
                    "prompt_block": nix_domain::prompt_block(&q, 900),
                }),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reports_honesty_and_counts() {
        let s = nix_domain::symbiosis_summary();
        assert!(s["lessons"].as_u64().unwrap_or(0) > 10);
        assert!(s["line"].as_str().is_some());
        assert_eq!(
            s["honesty"],
            "factos lidos do sistema; ausente = null, nunca estimado"
        );
    }

    #[test]
    fn ask_caps_are_none() {
        for m in ["nix.status", "nix.ask"] {
            let req = nos_ipc::Request {
                id: 1,
                method: m.to_string(),
                caps: 0,
                cap_gen: 0,
                agent: "test".into(),
                token: None,
                params: json!({}),
            };
            assert_eq!(req.required_cap(), nos_core::CapBits::NONE, "{m}");
        }
    }
}
