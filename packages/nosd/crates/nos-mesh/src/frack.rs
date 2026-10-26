//! FRACK\0 selective ACK (ADR-0081 Phase 2 — wire subset std).
//! Wire: `FRACK\0` (6) + stream_id u32 LE + seq u32 LE = 14 bytes.

pub const MAGIC: &[u8] = b"FRACK\0";
pub const HDR_LEN: usize = 6 + 4 + 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frack {
    pub stream_id: u32,
    pub seq: u32,
}

impl Frack {
    pub fn encode(self) -> Vec<u8> {
        let mut v = Vec::with_capacity(HDR_LEN);
        v.extend_from_slice(MAGIC);
        v.extend_from_slice(&self.stream_id.to_le_bytes());
        v.extend_from_slice(&self.seq.to_le_bytes());
        v
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < HDR_LEN || &buf[0..6] != MAGIC {
            return None;
        }
        let stream_id = u32::from_le_bytes(buf[6..10].try_into().ok()?);
        let seq = u32::from_le_bytes(buf[10..14].try_into().ok()?);
        Some(Self { stream_id, seq })
    }

    pub fn from_frag(msg_id: u32, frag_idx: u16) -> Self {
        Self {
            stream_id: msg_id,
            seq: u32::from(frag_idx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f = Frack {
            stream_id: 99,
            seq: 3,
        };
        let buf = f.encode();
        assert_eq!(Frack::parse(&buf), Some(f));
    }

    #[test]
    fn reject_bad_magic() {
        assert!(Frack::parse(b"FRAG\0").is_none());
    }
}
