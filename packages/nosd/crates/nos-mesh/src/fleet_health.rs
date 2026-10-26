//! FLEET_HEALTH / MCH worst-of (P.13 / #621).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthVerdict {
    Go,
    Unknown,
    NoGo,
}

impl HealthVerdict {
    pub fn worst(self, other: Self) -> Self {
        use HealthVerdict::*;
        match (self, other) {
            (NoGo, _) | (_, NoGo) => NoGo,
            (Unknown, _) | (_, Unknown) => Unknown,
            _ => Go,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineHealth {
    pub node_id: u8,
    pub sys: HealthVerdict,
    pub audio: HealthVerdict,
    pub hub: HealthVerdict,
    pub reason: String,
}

impl MachineHealth {
    pub fn aggregate(&self) -> HealthVerdict {
        self.sys.worst(self.audio).worst(self.hub)
    }
}

pub fn fleet_worst(machines: &[MachineHealth]) -> HealthVerdict {
    machines
        .iter()
        .map(|m| m.aggregate())
        .fold(HealthVerdict::Go, HealthVerdict::worst)
}

/// Wire MCH (bare-metal parity — SESSION_417 prefix contract).
pub const MAGIC_MCH: &[u8] = b"MCH\0";

pub fn encode_mch(h: &MachineHealth) -> Vec<u8> {
    let mut v = MAGIC_MCH.to_vec();
    if let Ok(json) = serde_json::to_vec(h) {
        v.extend_from_slice(&json);
    }
    v
}

pub fn parse_mch(buf: &[u8]) -> Option<MachineHealth> {
    if buf.len() <= MAGIC_MCH.len() || &buf[..MAGIC_MCH.len()] != MAGIC_MCH {
        return None;
    }
    serde_json::from_slice(&buf[MAGIC_MCH.len()..]).ok()
}

impl HealthVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::Unknown => "unknown",
            Self::NoGo => "no_go",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_contaminates() {
        assert_eq!(
            HealthVerdict::Go.worst(HealthVerdict::Unknown),
            HealthVerdict::Unknown
        );
    }

    #[test]
    fn mch_roundtrip() {
        let h = MachineHealth {
            node_id: 2,
            sys: HealthVerdict::Go,
            audio: HealthVerdict::Unknown,
            hub: HealthVerdict::Go,
            reason: "lab".into(),
        };
        let buf = encode_mch(&h);
        let p = parse_mch(&buf).unwrap();
        assert_eq!(p.node_id, 2);
        assert_eq!(p.aggregate(), HealthVerdict::Unknown);
    }

    #[test]
    fn fleet_nogo() {
        let ms = vec![
            MachineHealth {
                node_id: 1,
                sys: HealthVerdict::Go,
                audio: HealthVerdict::Go,
                hub: HealthVerdict::Go,
                reason: String::new(),
            },
            MachineHealth {
                node_id: 2,
                sys: HealthVerdict::NoGo,
                audio: HealthVerdict::Unknown,
                hub: HealthVerdict::Go,
                reason: "oom".into(),
            },
        ];
        assert_eq!(fleet_worst(&ms), HealthVerdict::NoGo);
    }
}
