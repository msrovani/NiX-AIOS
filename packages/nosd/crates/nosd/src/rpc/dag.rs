//! V9.1 (plano vampiro) — superfície `dag.plan`: valida o grafo **sem executar**.
//!
//! O contrato vive em `nos-core::dag` (puro); aqui só se converte JSON→nós e
//! se devolve o plano/erro. Não há efeito colateral: nenhum nó é executado.

use super::*;
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

fn strings(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|i| i.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    let _ = state;
    Some(match req.method.as_str() {
        "dag.plan" => {
            let arr = req
                .params
                .get("nodes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if arr.is_empty() {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some("dag.plan: nodes[] vazio".into()),
                    result: json!({ "layer_contract": true }),
                });
            }
            let nodes: Vec<nos_core::DagNode> = arr
                .iter()
                .map(|n| nos_core::DagNode {
                    id: n
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    deps: strings(n.get("deps")),
                    write_scope: strings(n.get("write_scope")),
                })
                .collect();
            match nos_core::dag_plan(&nodes) {
                Ok(p) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({
                        "layers": p.layers,
                        "node_count": p.node_count(),
                        "max_parallel": p.max_parallel(),
                        "write_scope": "disjoint por camada (prefixo por segmento)",
                        "executed": false,
                    }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "layer_contract": true, "executed": false }),
                },
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_trims_and_drops_empties() {
        assert_eq!(
            strings(Some(&json!([" a ", "", "b"]))),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(strings(None).is_empty());
        assert!(strings(Some(&json!(null))).is_empty());
    }
}
