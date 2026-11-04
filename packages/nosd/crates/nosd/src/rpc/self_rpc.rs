//! RPC — autoconhecimento (`self.status`) e pesquisa web (`net.research`).
//!
//! `self.status` responde à pergunta "quem sou, o que posso, o que não posso,
//! como sou feito, onde procurar" com a MESMA tabela que autoriza
//! (`nos_core::self_model` ↔ `nos_ipc::required_cap`, provado por teste).
//! `net.research` é o que o nó faz quando a resposta não está no sistema.

use super::*;
use nos_core::self_model::{self, SelfEnv};
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "self.status" => {
            let mut st = self_model::probe();
            // O daemon sabe do ring melhor do que o ambiente do processo.
            st.nix.ring_armed = state.nix_ring_armed;
            let backend = format!("{:?}", state.hermes.memory.backend()).to_lowercase();
            let env = SelfEnv {
                engine_present: Some(state.hermes.hotswap_s2.is_some()),
                model: None,
                // Rede não é sondada aqui: `None` = não verificado (honestidade).
                net: None,
                data_dir: Some(state.data_dir.display().to_string()),
                memory_backend: Some(backend.clone()),
            };
            let mut out = self_model::status(&st, &env);
            out["memory"] = json!({
                "healthy": state.hermes.memory.health(),
                "backend": backend,
            });
            out["prompt_block"] = json!(self_model::prompt_block(&st, &env, 700));
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: out,
            }
        }
        "net.research" => {
            let q = req
                .params
                .get("query")
                .or_else(|| req.params.get("url"))
                .or_else(|| req.params.get("q"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if q.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("params.query (ou url) required".into()),
                    result: json!(null),
                });
            }
            let k = req
                .params
                .get("k")
                .and_then(|v| v.as_u64())
                .unwrap_or(5)
                .clamp(1, 8) as usize;
            let r = nos_cortex::research(&q, k);
            tracing::info!(
                target: "nosd",
                "net.research kind={} sources={} error={:?}",
                r.kind,
                r.sources.len(),
                r.error
            );
            Response {
                id: req.id,
                ok: r.error.is_none(),
                error: r.error.clone(),
                result: json!({
                    "research": r,
                    "honesty": "fonte externa citada; sem resposta = no_answer/net_unavailable, nunca inventado",
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
    fn caps_are_read_only_for_self_and_net_for_research() {
        let mk = |m: &str| Request {
            id: 1,
            method: m.to_string(),
            caps: 0,
            cap_gen: 0,
            agent: "test".into(),
            token: None,
            params: json!({}),
        };
        assert_eq!(mk("self.status").required_cap(), nos_core::CapBits::NONE);
        assert_eq!(mk("net.research").required_cap(), nos_core::CapBits::NET);
    }

    #[test]
    fn research_requires_a_query() {
        // Sem params → erro explícito (nunca pesquisa vazia).
        let r = nos_cortex::research("", 3);
        assert!(r.error.is_some());
    }
}
