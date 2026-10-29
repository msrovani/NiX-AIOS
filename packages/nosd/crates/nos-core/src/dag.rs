//! V9.1 (plano vampiro) — plano DAG com **isolamento de write-scope**.
//!
//! Origem da ideia: `oh-my-hermes` (`/dag-ulw`: "nodes running at once never
//! share a write scope"). Aqui é um **contrato puro**: dado um grafo de nós com
//! dependências e escopos de escrita, devolve as camadas topológicas e recusa
//! qualquer camada onde dois nós possam escrever no mesmo sítio.
//!
//! Regras (fail-closed):
//! - id vazio, duplicado, dependência desconhecida ou auto-dependência = erro;
//! - ciclo = erro com os nós presos (não inventa ordem);
//! - `write_scope` vazio = nó **read-only declarado** (pode coexistir com tudo);
//! - conflito de escopo é por segmento: `src/a` conflita com `src/a/b`, mas não
//!   com `src/ab`;
//! - a ordem dentro de cada camada é determinística (lexicográfica).
//!
//! Ainda **não** está ligado a `job.submit`/factory: execução é passo seguinte.

use std::collections::{BTreeMap, BTreeSet};

/// Limite duro de nós por plano (evita explosão de memória num pedido IPC).
pub const MAX_NODES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagNode {
    pub id: String,
    pub deps: Vec<String>,
    /// Escopos de escrita (caminhos/prefixos). Vazio = read-only declarado.
    pub write_scope: Vec<String>,
}

impl DagNode {
    pub fn new(id: impl Into<String>, deps: &[&str], write_scope: &[&str]) -> Self {
        Self {
            id: id.into(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
            write_scope: write_scope.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagPlan {
    /// Camadas em ordem topológica; nós da mesma camada correm em paralelo.
    pub layers: Vec<Vec<String>>,
}

impl DagPlan {
    pub fn node_count(&self) -> usize {
        self.layers.iter().map(|l| l.len()).sum()
    }

    pub fn max_parallel(&self) -> usize {
        self.layers.iter().map(|l| l.len()).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    Empty,
    TooLarge(usize),
    EmptyId,
    DuplicateNode(String),
    UnknownDep {
        node: String,
        dep: String,
    },
    EmptyScopeEntry(String),
    Cycle(Vec<String>),
    WriteConflict {
        layer: usize,
        a: String,
        b: String,
        scope: String,
    },
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "dag: sem nós"),
            Self::TooLarge(n) => write!(f, "dag: {n} nós (max {MAX_NODES})"),
            Self::EmptyId => write!(f, "dag: nó com id vazio"),
            Self::DuplicateNode(id) => write!(f, "dag: nó duplicado {id}"),
            Self::UnknownDep { node, dep } => {
                write!(f, "dag: {node} depende de {dep} (desconhecido)")
            }
            Self::EmptyScopeEntry(id) => {
                write!(f, "dag: nó {id} com write_scope vazio entre entradas")
            }
            Self::Cycle(nodes) => write!(f, "dag: ciclo entre {nodes:?}"),
            Self::WriteConflict { layer, a, b, scope } => write!(
                f,
                "dag: camada {layer}: {a} e {b} escrevem em {scope} (isolamento violado)"
            ),
        }
    }
}

impl std::error::Error for DagError {}

/// Normaliza um escopo: trim, barras finais removidas, vazio = inválido.
fn norm_scope(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let t = t.trim_end_matches('/');
    if t.is_empty() {
        return None;
    }
    Some(t.to_string())
}

/// Dois escopos colidem se forem iguais ou um for prefixo do outro por segmento.
pub fn scopes_conflict(a: &str, b: &str) -> bool {
    match (norm_scope(a), norm_scope(b)) {
        (Some(a), Some(b)) => {
            a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
        }
        // Escopo inválido nunca "colide" (é recusado antes, na validação).
        _ => false,
    }
}

fn layer_conflict(
    layer: usize,
    nodes: &BTreeMap<String, &DagNode>,
    ids: &[String],
) -> Option<DagError> {
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let (na, nb) = (&ids[i], &ids[j]);
            let (Some(a), Some(b)) = (nodes.get(na), nodes.get(nb)) else {
                continue;
            };
            for sa in &a.write_scope {
                for sb in &b.write_scope {
                    if scopes_conflict(sa, sb) {
                        return Some(DagError::WriteConflict {
                            layer,
                            a: na.clone(),
                            b: nb.clone(),
                            scope: norm_scope(sa).unwrap_or_else(|| sa.clone()),
                        });
                    }
                }
            }
        }
    }
    None
}

/// Calcula as camadas topológicas com isolamento de write-scope.
pub fn plan(nodes: &[DagNode]) -> Result<DagPlan, DagError> {
    if nodes.is_empty() {
        return Err(DagError::Empty);
    }
    if nodes.len() > MAX_NODES {
        return Err(DagError::TooLarge(nodes.len()));
    }

    let mut by_id: BTreeMap<String, &DagNode> = BTreeMap::new();
    for n in nodes {
        let id = n.id.trim();
        if id.is_empty() {
            return Err(DagError::EmptyId);
        }
        if id != n.id {
            return Err(DagError::DuplicateNode(format!("{id} (com espaços)")));
        }
        if by_id.insert(n.id.clone(), n).is_some() {
            return Err(DagError::DuplicateNode(n.id.clone()));
        }
        for s in &n.write_scope {
            if norm_scope(s).is_none() {
                return Err(DagError::EmptyScopeEntry(n.id.clone()));
            }
        }
    }
    for n in nodes {
        for d in &n.deps {
            if !by_id.contains_key(d) {
                return Err(DagError::UnknownDep {
                    node: n.id.clone(),
                    dep: d.clone(),
                });
            }
        }
    }

    // Kahn por camadas.
    let mut indegree: BTreeMap<String, usize> = by_id.keys().map(|k| (k.clone(), 0usize)).collect();
    let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        for d in &n.deps {
            *indegree.get_mut(&n.id).expect("nó validado") += 1;
            dependents
                .entry(d.clone())
                .or_default()
                .insert(n.id.clone());
        }
    }

    let mut layers: Vec<Vec<String>> = Vec::new();
    let mut done: BTreeSet<String> = BTreeSet::new();
    loop {
        let mut ready: Vec<String> = indegree
            .iter()
            .filter(|(id, deg)| **deg == 0 && !done.contains(*id))
            .map(|(id, _)| id.clone())
            .collect();
        ready.sort();
        if ready.is_empty() {
            break;
        }
        if let Some(e) = layer_conflict(layers.len(), &by_id, &ready) {
            return Err(e);
        }
        for id in &ready {
            done.insert(id.clone());
            for dep in dependents.get(id).into_iter().flatten() {
                if let Some(d) = indegree.get_mut(dep) {
                    *d = d.saturating_sub(1);
                }
            }
        }
        layers.push(ready);
    }

    if done.len() != nodes.len() {
        let stuck: Vec<String> = by_id
            .keys()
            .filter(|k| !done.contains(*k))
            .cloned()
            .collect();
        return Err(DagError::Cycle(stuck));
    }

    Ok(DagPlan { layers })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(layers: &[Vec<String>]) -> Vec<Vec<&str>> {
        layers
            .iter()
            .map(|l| l.iter().map(String::as_str).collect())
            .collect()
    }

    #[test]
    fn chain_becomes_sequential_layers() {
        let p = plan(&[
            DagNode::new("c", &["b"], &["src/c"]),
            DagNode::new("a", &[], &["src/a"]),
            DagNode::new("b", &["a"], &["src/b"]),
        ])
        .unwrap();
        assert_eq!(ids(&p.layers), vec![vec!["a"], vec!["b"], vec!["c"]]);
        assert_eq!(p.max_parallel(), 1);
    }

    #[test]
    fn independent_nodes_share_a_layer_sorted() {
        let p = plan(&[
            DagNode::new("zeta", &[], &["src/z"]),
            DagNode::new("alpha", &[], &["src/a"]),
            DagNode::new("mid", &[], &[]),
        ])
        .unwrap();
        assert_eq!(ids(&p.layers), vec![vec!["alpha", "mid", "zeta"]]);
        assert_eq!(p.max_parallel(), 3);
        assert_eq!(p.node_count(), 3);
    }

    #[test]
    fn same_layer_write_conflict_is_refused() {
        let e = plan(&[
            DagNode::new("a", &[], &["src/rpc"]),
            DagNode::new("b", &[], &["src/rpc"]),
        ])
        .unwrap_err();
        match e {
            DagError::WriteConflict { layer, scope, .. } => {
                assert_eq!(layer, 0);
                assert_eq!(scope, "src/rpc");
            }
            other => panic!("esperava WriteConflict, veio {other:?}"),
        }
    }

    #[test]
    fn prefix_conflicts_but_sibling_names_do_not() {
        assert!(scopes_conflict("src/a", "src/a/b"));
        assert!(scopes_conflict("src/a/", "src/a"));
        assert!(!scopes_conflict("src/a", "src/ab"));
        assert!(!scopes_conflict("src/a", "lib/a"));

        let e = plan(&[
            DagNode::new("pai", &[], &["src"]),
            DagNode::new("filho", &[], &["src/x"]),
        ])
        .unwrap_err();
        assert!(matches!(e, DagError::WriteConflict { .. }));
    }

    #[test]
    fn read_only_nodes_coexist_with_writers() {
        let p = plan(&[
            DagNode::new("writer", &[], &["src/x"]),
            DagNode::new("reader", &[], &[]),
        ])
        .unwrap();
        assert_eq!(ids(&p.layers), vec![vec!["reader", "writer"]]);
    }

    #[test]
    fn conflict_across_layers_is_allowed() {
        // b depende de a: nunca correm juntos, logo o mesmo escopo é legítimo.
        let p = plan(&[
            DagNode::new("a", &[], &["src/x"]),
            DagNode::new("b", &["a"], &["src/x"]),
        ])
        .unwrap();
        assert_eq!(ids(&p.layers), vec![vec!["a"], vec!["b"]]);
    }

    #[test]
    fn cycle_is_reported_with_the_stuck_nodes() {
        let e = plan(&[
            DagNode::new("a", &["b"], &[]),
            DagNode::new("b", &["a"], &[]),
        ])
        .unwrap_err();
        match e {
            DagError::Cycle(v) => assert_eq!(v, vec!["a".to_string(), "b".to_string()]),
            other => panic!("esperava Cycle, veio {other:?}"),
        }
    }

    #[test]
    fn validation_fails_closed() {
        assert_eq!(plan(&[]), Err(DagError::Empty));
        assert_eq!(plan(&[DagNode::new("", &[], &[])]), Err(DagError::EmptyId));
        assert_eq!(
            plan(&[DagNode::new("a", &[], &[]), DagNode::new("a", &[], &[])]),
            Err(DagError::DuplicateNode("a".into()))
        );
        assert_eq!(
            plan(&[DagNode::new("a", &["x"], &[])]),
            Err(DagError::UnknownDep {
                node: "a".into(),
                dep: "x".into()
            })
        );
        assert_eq!(
            plan(&[DagNode::new("a", &[], &["  "])]),
            Err(DagError::EmptyScopeEntry("a".into()))
        );
    }

    #[test]
    fn too_many_nodes_is_refused() {
        let nodes: Vec<DagNode> = (0..=MAX_NODES)
            .map(|i| DagNode::new(format!("n{i}"), &[], &[]))
            .collect();
        assert_eq!(plan(&nodes), Err(DagError::TooLarge(MAX_NODES + 1)));
    }

    #[test]
    fn diamond_has_three_layers() {
        let p = plan(&[
            DagNode::new("root", &[], &[]),
            DagNode::new("l", &["root"], &["src/l"]),
            DagNode::new("r", &["root"], &["src/r"]),
            DagNode::new("join", &["l", "r"], &["src/join"]),
        ])
        .unwrap();
        assert_eq!(
            ids(&p.layers),
            vec![vec!["root"], vec!["l", "r"], vec!["join"]]
        );
    }
}
