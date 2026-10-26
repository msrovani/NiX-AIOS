//! Wire `LayaIntent` / intent hint (NoProto `TaskType::LayaIntent = 8` — protocolo only).
//!
//! **CRDT / scorer destilado: NOT_DONE** — pacote transporta hint; merge e triagem
//! fail-closed ficam no bare-metal quando #611 fechar (ADR-0110).

use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8] = b"LI\0";
pub const WIRE_VERSION: u8 = 1;

/// Hint de intenção leve para mesh (não substitui `decide_intent` no tick).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayaIntent {
    pub node_id: u32,
    pub clock: u64,
    /// Braço 0..14 (espelha `decide_intent` arms — sem validação aqui).
    pub intent_arm: u8,
    pub confidence_q8: u8,
    pub margin_q8: u8,
    /// FNV-1a64 do texto normalizado (opcional; 0 = ausente).
    pub text_fnv: u64,
}

impl LayaIntent {
    pub fn encode(&self) -> Vec<u8> {
        let mut v = MAGIC.to_vec();
        v.push(WIRE_VERSION);
        v.extend_from_slice(&self.node_id.to_le_bytes());
        v.extend_from_slice(&self.clock.to_le_bytes());
        v.push(self.intent_arm);
        v.push(self.confidence_q8);
        v.push(self.margin_q8);
        v.extend_from_slice(&self.text_fnv.to_le_bytes());
        v
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 3 + 1 + 4 + 8 + 3 + 8 || &buf[0..3] != MAGIC {
            return None;
        }
        if buf[3] != WIRE_VERSION {
            return None;
        }
        let mut o = 4usize;
        let node_id = read_u32(buf, &mut o)?;
        let clock = read_u64(buf, &mut o)?;
        let intent_arm = *buf.get(o)?;
        o += 1;
        let confidence_q8 = *buf.get(o)?;
        o += 1;
        let margin_q8 = *buf.get(o)?;
        o += 1;
        let text_fnv = read_u64(buf, &mut o)?;
        Some(Self {
            node_id,
            clock,
            intent_arm,
            confidence_q8,
            margin_q8,
            text_fnv,
        })
    }
}

fn read_u32(buf: &[u8], o: &mut usize) -> Option<u32> {
    let end = o.saturating_add(4);
    let bytes: [u8; 4] = buf.get(*o..end)?.try_into().ok()?;
    *o = end;
    Some(u32::from_le_bytes(bytes))
}

fn read_u64(buf: &[u8], o: &mut usize) -> Option<u64> {
    let end = o.saturating_add(8);
    let bytes: [u8; 8] = buf.get(*o..end)?.try_into().ok()?;
    *o = end;
    Some(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let li = LayaIntent {
            node_id: 15,
            clock: 42,
            intent_arm: 3,
            confidence_q8: 200,
            margin_q8: 40,
            text_fnv: 0xdead_beef_cafe_babe,
        };
        let enc = li.encode();
        let dec = LayaIntent::decode(&enc).expect("decode");
        assert_eq!(dec, li);
    }

    #[test]
    fn reject_bad_magic() {
        assert!(LayaIntent::decode(b"XX\0").is_none());
    }
}
