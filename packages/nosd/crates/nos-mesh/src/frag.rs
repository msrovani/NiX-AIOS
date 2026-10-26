//! FRAG\0 reassembly (PORT contrato ADR-0081 — subset std, CAP 8 slots).
//! Header: `FRAG\0` (5) + msg_id u32 LE + frag_idx u16 LE + frag_total u16 LE + payload.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAGIC: &[u8] = b"FRAG\0";
pub const HDR_LEN: usize = 5 + 4 + 2 + 2; // 13
const MAX_SLOTS: usize = 8;
const MAX_FRAGS: u16 = 64;
const MAX_PAYLOAD: usize = 1200;
const TTL_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragHeader {
    pub msg_id: u32,
    pub frag_idx: u16,
    pub frag_total: u16,
}

#[derive(Debug)]
struct Slot {
    msg_id: u32,
    total: u16,
    parts: HashMap<u16, Vec<u8>>,
    last_ms: u64,
}

#[derive(Default)]
pub struct FragReassembler {
    slots: Mutex<Vec<Slot>>,
}

impl FragReassembler {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(Vec::with_capacity(MAX_SLOTS)),
        }
    }

    /// Encodifica um fragmento. `frag_total` ≥ 1; idx < total.
    pub fn encode(msg_id: u32, frag_idx: u16, frag_total: u16, payload: &[u8]) -> Option<Vec<u8>> {
        if frag_total == 0 || frag_idx >= frag_total || frag_total > MAX_FRAGS {
            return None;
        }
        if payload.len() > MAX_PAYLOAD {
            return None;
        }
        let mut v = Vec::with_capacity(HDR_LEN + payload.len());
        v.extend_from_slice(MAGIC);
        v.extend_from_slice(&msg_id.to_le_bytes());
        v.extend_from_slice(&frag_idx.to_le_bytes());
        v.extend_from_slice(&frag_total.to_le_bytes());
        v.extend_from_slice(payload);
        Some(v)
    }

    pub fn parse(buf: &[u8]) -> Option<(FragHeader, &[u8])> {
        if buf.len() < HDR_LEN || &buf[0..5] != MAGIC {
            return None;
        }
        let msg_id = u32::from_le_bytes(buf[5..9].try_into().ok()?);
        let frag_idx = u16::from_le_bytes(buf[9..11].try_into().ok()?);
        let frag_total = u16::from_le_bytes(buf[11..13].try_into().ok()?);
        if frag_total == 0 || frag_idx >= frag_total || frag_total > MAX_FRAGS {
            return None;
        }
        Some((
            FragHeader {
                msg_id,
                frag_idx,
                frag_total,
            },
            &buf[HDR_LEN..],
        ))
    }

    /// Injeta fragmento; devolve payload completo quando todos chegaram.
    pub fn push(&self, buf: &[u8]) -> Option<Vec<u8>> {
        let (h, payload) = Self::parse(buf)?;
        let now = now_ms();
        let mut slots = self.slots.lock();
        Self::evict_expired(&mut slots, now);

        let pos = slots.iter().position(|s| s.msg_id == h.msg_id);
        let idx = match pos {
            Some(i) => i,
            None => {
                if slots.len() >= MAX_SLOTS {
                    // evict oldest
                    if let Some((oi, _)) = slots
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, s)| s.last_ms)
                    {
                        slots.remove(oi);
                    }
                }
                slots.push(Slot {
                    msg_id: h.msg_id,
                    total: h.frag_total,
                    parts: HashMap::new(),
                    last_ms: now,
                });
                slots.len() - 1
            }
        };
        let slot = &mut slots[idx];
        if slot.total != h.frag_total {
            return None;
        }
        slot.last_ms = now;
        slot.parts.insert(h.frag_idx, payload.to_vec());
        if slot.parts.len() as u16 != slot.total {
            return None;
        }
        let mut out = Vec::new();
        for i in 0..slot.total {
            out.extend_from_slice(slot.parts.get(&i)?);
        }
        slots.remove(idx);
        Some(out)
    }

    /// Parte um payload em fragmentos ≤ `chunk` bytes.
    pub fn split(msg_id: u32, payload: &[u8], chunk: usize) -> Vec<Vec<u8>> {
        let chunk = chunk.clamp(1, MAX_PAYLOAD);
        if payload.is_empty() {
            return Self::encode(msg_id, 0, 1, &[]).into_iter().collect();
        }
        let parts: Vec<_> = payload.chunks(chunk).collect();
        let total = parts.len() as u16;
        parts
            .into_iter()
            .enumerate()
            .filter_map(|(i, p)| Self::encode(msg_id, i as u16, total, p))
            .collect()
    }

    fn evict_expired(slots: &mut Vec<Slot>, now: u64) {
        slots.retain(|s| now.saturating_sub(s.last_ms) < TTL_MS);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_three() {
        let r = FragReassembler::new();
        let payload = b"hello-mesh-frag-payload-0123456789";
        let parts = FragReassembler::split(42, payload, 10);
        assert!(parts.len() >= 3);
        let mut done = None;
        for p in &parts {
            if let Some(v) = r.push(p) {
                done = Some(v);
            }
        }
        assert_eq!(done.as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn reject_bad_magic() {
        assert!(FragReassembler::parse(b"XXXX\0").is_none());
    }
}
