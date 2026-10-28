//! CRDT thin (M.2) — last-writer-wins por chave com vector clock por peer.
//! Merge in-process para lab; sync P2P NSGDB pleno = residual AWAITING.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Relógio lógico por `peer_id` (versão monotónica).
pub type VersionVector = HashMap<u32, u64>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrdtEntry {
    pub key: String,
    pub value: String,
    pub vv: VersionVector,
    /// Desempate estável quando VV concurrent.
    pub writer: u32,
    pub tick: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrdtStore {
    pub entries: HashMap<String, CrdtEntry>,
}

impl CrdtStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Put local: incrementa o componente `peer` no VV.
    ///
    /// O contador tem de ser **global por peer** (max em todas as entradas),
    /// nao por chave: senao a 2a chave nova de um no nasce com o mesmo
    /// `vv[peer]` da 1a e `delta_since` esconde-a de quem ja viu a 1a.
    pub fn put(&mut self, peer: u32, key: impl Into<String>, value: impl Into<String>, tick: u64) {
        let key = key.into();
        let mut vv = self
            .entries
            .get(&key)
            .map(|e| e.vv.clone())
            .unwrap_or_default();
        let next = self
            .version_vector()
            .get(&peer)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        vv.insert(peer, next);
        self.entries.insert(
            key.clone(),
            CrdtEntry {
                key,
                value: value.into(),
                vv,
                writer: peer,
                tick,
            },
        );
    }

    pub fn get(&self, key: &str) -> Option<&CrdtEntry> {
        self.entries.get(key)
    }

    /// Merge LWW: se `remote` domina VV → adopt; se concurrent → maior (tick, writer).
    pub fn merge_entry(&mut self, remote: CrdtEntry) {
        match self.entries.get(&remote.key) {
            None => {
                self.entries.insert(remote.key.clone(), remote);
            }
            Some(local) => {
                match vv_compare(&local.vv, &remote.vv) {
                    VvOrd::Less => {
                        self.entries.insert(remote.key.clone(), remote);
                    }
                    VvOrd::Greater => { /* keep local */ }
                    VvOrd::Concurrent | VvOrd::Equal => {
                        let take_remote = (remote.tick, remote.writer) > (local.tick, local.writer);
                        if take_remote {
                            self.entries.insert(remote.key.clone(), remote);
                        }
                    }
                }
            }
        }
    }

    pub fn merge_store(&mut self, other: &CrdtStore) {
        for e in other.entries.values() {
            self.merge_entry(e.clone());
        }
    }

    /// M8.1: VV agregado (max por peer em todas as entradas).
    pub fn version_vector(&self) -> VersionVector {
        let mut vv = VersionVector::new();
        for e in self.entries.values() {
            for (p, c) in &e.vv {
                let s = vv.entry(*p).or_insert(0);
                if *c > *s {
                    *s = *c;
                }
            }
        }
        vv
    }

    /// M8.1: entradas que quem tem `since` ainda nao viu.
    pub fn delta_since(&self, since: &VersionVector) -> Vec<CrdtEntry> {
        self.entries
            .values()
            .filter(|e| {
                e.vv.iter()
                    .any(|(p, c)| *c > since.get(p).copied().unwrap_or(0))
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VvOrd {
    Less,
    Greater,
    Concurrent,
    Equal,
}

fn vv_compare(a: &VersionVector, b: &VersionVector) -> VvOrd {
    let mut a_gt = false;
    let mut b_gt = false;
    let mut keys: Vec<u32> = a.keys().copied().chain(b.keys().copied()).collect();
    keys.sort_unstable();
    keys.dedup();
    for k in keys {
        let av = a.get(&k).copied().unwrap_or(0);
        let bv = b.get(&k).copied().unwrap_or(0);
        if av > bv {
            a_gt = true;
        } else if bv > av {
            b_gt = true;
        }
    }
    match (a_gt, b_gt) {
        (false, false) => VvOrd::Equal,
        (true, false) => VvOrd::Greater,
        (false, true) => VvOrd::Less,
        (true, true) => VvOrd::Concurrent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_peers_lww_merge() {
        let mut a = CrdtStore::new();
        let mut b = CrdtStore::new();
        a.put(1, "soul", "calm", 10);
        b.put(2, "soul", "alert", 20);
        // Concurrent puts — higher tick wins
        a.merge_store(&b);
        b.merge_store(&a);
        assert_eq!(a.get("soul").unwrap().value, "alert");
        assert_eq!(b.get("soul").unwrap().value, "alert");
    }

    #[test]
    fn second_key_of_same_writer_is_in_delta() {
        // Regressao M8.1: o VV tem de ser global por peer. Com contador por
        // chave, `k2` nascia com vv[1]=1 e ficava fora do delta de quem ja
        // tinha vv[1]=1 ? a frota nunca a recebia.
        let mut a = CrdtStore::new();
        a.put(1, "k1", "v1", 0);
        let seen = a.version_vector();
        a.put(1, "k2", "v2", 1);
        let delta = a.delta_since(&seen);
        assert_eq!(delta.len(), 1, "{delta:?}");
        assert_eq!(delta[0].key, "k2");
        assert_eq!(a.version_vector().get(&1).copied(), Some(2));
    }

    #[test]
    fn causal_overwrite() {
        let mut a = CrdtStore::new();
        a.put(1, "k", "v1", 1);
        let mut b = a.clone();
        b.put(1, "k", "v2", 2); // same peer advances VV
        a.merge_store(&b);
        assert_eq!(a.get("k").unwrap().value, "v2");
    }
}
