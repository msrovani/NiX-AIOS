//! Mesh P2P — contratos wire ADR-0081 (PORT sobre UDP std).

pub mod crdt;
pub mod fleet_health;
pub mod frack;
pub mod frag;
pub mod laya_intent;
pub mod mesh_knowledge;
pub mod soul_mirror;
pub mod udp;

pub use crdt::{CrdtEntry, CrdtStore, VersionVector};
pub use fleet_health::{
    encode_mch, fleet_worst, parse_mch, HealthVerdict, MachineHealth, MAGIC_MCH,
};
pub use frack::Frack;
pub use frag::{FragHeader, FragReassembler};
pub use laya_intent::{LayaIntent, WIRE_VERSION as LAYA_WIRE_VERSION};
pub use mesh_knowledge::{content_fingerprint, fnv1a64, payload_fingerprint};
pub use soul_mirror::SoulMirrorSnapshot;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const MAGIC_HB: &[u8] = b"HB\0";
pub const MAGIC_FRAG: &[u8] = frag::MAGIC;
pub const MAGIC_ROLE: &[u8] = b"ROLE\0";
/// Janela mínima após 1º peer PK-bound antes de ROLE / SkillSync (ADR-0081).
pub const TOFU_SETTLE_TICKS: u32 = 130;
/// Equivalente ~130 ticks @ ~18 Hz quando `tick()` não corre.
pub const TOFU_SETTLE_MS: u64 = 7_200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerRole {
    Master,
    Worker,
    Memory,
    Compute,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapacityScore {
    pub cpu_units: u32,
    pub mem_mib: u32,
    pub net_score: u32,
}

impl CapacityScore {
    pub fn total(&self) -> u64 {
        u64::from(self.cpu_units) * 10_000 + u64::from(self.mem_mib) + u64::from(self.net_score)
    }
}

impl Default for CapacityScore {
    fn default() -> Self {
        Self {
            cpu_units: 4,
            mem_mib: 2048,
            net_score: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: u32,
    pub last_seen_ms: u64,
    pub pk_bound: bool,
    /// Quando `pk_bound` ficou true (ms epoch) — início da janela TOFU.
    pub pk_bound_at_ms: Option<u64>,
    pub capacity: CapacityScore,
    pub role: PeerRole,
}

#[derive(Clone)]
pub struct MeshEngine {
    peers: Arc<Mutex<HashMap<u32, PeerInfo>>>,
    local_id: u32,
    local_capacity: CapacityScore,
    local_role: Arc<Mutex<PeerRole>>,
    tofu_ticks: Arc<Mutex<u32>>,
    first_pk_bound_ms: Arc<Mutex<Option<u64>>>,
    frags: Arc<FragReassembler>,
    /// Último payload FRAG reassembled (lab/IPC).
    last_frag: Arc<Mutex<Option<Vec<u8>>>>,
    /// ACKs a enviar após receber FRAG (stop-and-wait lab).
    pending_fracks: Arc<Mutex<Vec<Frack>>>,
    fracks_received: Arc<Mutex<Vec<Frack>>>,
    /// Saúde local para FLEET worst-of (HUD / RPC).
    local_fleet: Arc<Mutex<Option<MachineHealth>>>,
    /// Saúde reportada por peers (MCH wire).
    peer_fleet: Arc<Mutex<HashMap<u32, MachineHealth>>>,
    /// Último LayaIntent recebido ou emitido (lab).
    last_laya: Arc<Mutex<Option<LayaIntent>>>,
    /// CRDT thin local (SoulMirror / M.2) — sync P2P residual.
    crdt: Arc<Mutex<CrdtStore>>,
}

impl Default for MeshEngine {
    fn default() -> Self {
        Self::new(15)
    }
}

impl MeshEngine {
    pub fn new(local_id: u32) -> Self {
        Self::with_capacity(local_id, CapacityScore::default())
    }

    pub fn with_capacity(local_id: u32, capacity: CapacityScore) -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            local_id,
            local_capacity: capacity,
            local_role: Arc::new(Mutex::new(PeerRole::Worker)),
            tofu_ticks: Arc::new(Mutex::new(0)),
            first_pk_bound_ms: Arc::new(Mutex::new(None)),
            frags: Arc::new(FragReassembler::new()),
            last_frag: Arc::new(Mutex::new(None)),
            pending_fracks: Arc::new(Mutex::new(Vec::new())),
            fracks_received: Arc::new(Mutex::new(Vec::new())),
            local_fleet: Arc::new(Mutex::new(None)),
            peer_fleet: Arc::new(Mutex::new(HashMap::new())),
            last_laya: Arc::new(Mutex::new(None)),
            crdt: Arc::new(Mutex::new(CrdtStore::new())),
        }
    }

    pub fn local_id(&self) -> u32 {
        self.local_id
    }

    pub fn crdt_put_soul(&self, snap: &SoulMirrorSnapshot) {
        let mut store = self.crdt.lock();
        snap.into_crdt(&mut store);
    }

    pub fn crdt_get(&self, key: &str) -> Option<CrdtEntry> {
        self.crdt.lock().get(key).cloned()
    }

    pub fn on_heartbeat(&self, peer_id: u32, has_pk: bool) {
        self.on_heartbeat_inner(peer_id, has_pk, None);
    }

    pub fn on_heartbeat_with_capacity(&self, peer_id: u32, has_pk: bool, capacity: CapacityScore) {
        self.on_heartbeat_inner(peer_id, has_pk, Some(capacity));
    }

    fn on_heartbeat_inner(&self, peer_id: u32, has_pk: bool, capacity: Option<CapacityScore>) {
        let now = now_ms();
        let mut g = self.peers.lock();
        let e = g.entry(peer_id).or_insert(PeerInfo {
            id: peer_id,
            last_seen_ms: now,
            pk_bound: false,
            pk_bound_at_ms: None,
            capacity: capacity.unwrap_or_default(),
            role: PeerRole::Worker,
        });
        e.last_seen_ms = now;
        if let Some(c) = capacity {
            e.capacity = c;
        }
        if has_pk && !e.pk_bound {
            e.pk_bound = true;
            e.pk_bound_at_ms = Some(now);
            let mut first = self.first_pk_bound_ms.lock();
            if first.is_none() {
                *first = Some(now);
            }
        } else if has_pk {
            e.pk_bound = true;
        }
        info!(target: "nosd", "mesh HB peer={} pk={}", peer_id, e.pk_bound);
        drop(g);
        self.run_election_stub();
    }

    /// Avança contador de settle (chamar no tick Hermes / mesh loop).
    pub fn tick(&self) {
        *self.tofu_ticks.lock() += 1;
    }

    pub fn tofu_ticks(&self) -> u32 {
        *self.tofu_ticks.lock()
    }

    pub fn peer_tofu_count(&self) -> usize {
        self.peers.lock().values().filter(|p| p.pk_bound).count()
    }

    /// ROLE / SkillSync (FRAG lab) só após janela TOFU (130 ticks ou `TOFU_SETTLE_MS`).
    pub fn may_send_control_sync(&self) -> bool {
        self.tofu_settled()
    }

    /// Eleição stub: maior `CapacityScore::total` vira Master; demais Worker.
    pub fn run_election_stub(&self) -> PeerRole {
        let mut best_id = self.local_id;
        let mut best = self.local_capacity.total();
        {
            let g = self.peers.lock();
            for p in g.values() {
                let t = p.capacity.total();
                if t > best || (t == best && p.id < best_id) {
                    best = t;
                    best_id = p.id;
                }
            }
        }
        let role = if best_id == self.local_id {
            PeerRole::Master
        } else {
            PeerRole::Worker
        };
        *self.local_role.lock() = role;
        let mut g = self.peers.lock();
        for p in g.values_mut() {
            p.role = if p.id == best_id {
                PeerRole::Master
            } else {
                PeerRole::Worker
            };
        }
        role
    }

    pub fn local_role(&self) -> PeerRole {
        *self.local_role.lock()
    }

    pub fn local_capacity(&self) -> CapacityScore {
        self.local_capacity
    }

    pub fn peer_role(&self, peer_id: u32) -> Option<PeerRole> {
        self.peers.lock().get(&peer_id).map(|p| p.role)
    }

    /// True quando há ≥1 peer PK-bound e passou a janela desde o 1º bind.
    pub fn tofu_settled(&self) -> bool {
        if self.peer_tofu_count() == 0 {
            return false;
        }
        if self.tofu_ticks() >= TOFU_SETTLE_TICKS {
            return true;
        }
        let first = *self.first_pk_bound_ms.lock();
        match first {
            Some(t0) => now_ms().saturating_sub(t0) >= TOFU_SETTLE_MS,
            None => false,
        }
    }

    pub fn peer_count(&self) -> u32 {
        self.peers.lock().len() as u32
    }

    pub fn set_local_fleet_health(&self, h: MachineHealth) {
        *self.local_fleet.lock() = Some(h);
    }

    pub fn encode_laya_intent(&self, intent: &LayaIntent) -> Vec<u8> {
        intent.encode()
    }

    /// Agenda LayaIntent para TX no próximo `MeshUdp::tick_tx` (M.2 residual).
    pub fn queue_laya_intent(&self, intent: LayaIntent) {
        *self.last_laya.lock() = Some(intent);
    }

    pub fn last_laya_intent(&self) -> Option<LayaIntent> {
        self.last_laya.lock().clone()
    }

    /// Peers → `nos_cortex::PeerCapacity` shape (M.4 offload).
    pub fn peers_for_offload(&self) -> Vec<(u32, u64, u64, bool)> {
        self.peers
            .lock()
            .values()
            .map(|p| {
                let unreachable = now_ms().saturating_sub(p.last_seen_ms) > 60_000;
                (
                    p.id,
                    u64::from(p.capacity.mem_mib),
                    u64::from(p.capacity.cpu_units),
                    unreachable,
                )
            })
            .collect()
    }

    /// Datagramas auxiliares a enviar com o HB (MCH + LayaIntent).
    pub fn encode_tick_extras(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if let Some(ref h) = *self.local_fleet.lock() {
            out.push(encode_mch(h));
        }
        if let Some(li) = self.last_laya_intent() {
            out.push(li.encode());
        }
        out
    }

    /// Máquinas para `fleet_worst` (local + peers com MCH).
    pub fn fleet_machines(&self) -> Vec<MachineHealth> {
        let mut out = Vec::new();
        if let Some(ref h) = *self.local_fleet.lock() {
            out.push(h.clone());
        }
        for h in self.peer_fleet.lock().values() {
            out.push(h.clone());
        }
        out
    }

    pub fn fleet_verdict(&self) -> Option<HealthVerdict> {
        let ms = self.fleet_machines();
        if ms.is_empty() {
            None
        } else {
            Some(fleet_worst(&ms))
        }
    }

    pub fn encode_heartbeat(&self) -> Vec<u8> {
        let mut v = MAGIC_HB.to_vec();
        v.extend_from_slice(&self.local_id.to_le_bytes());
        v.extend_from_slice(b"PK\0");
        v.extend_from_slice(b"CAP\0");
        v.extend_from_slice(&self.local_capacity.cpu_units.to_le_bytes());
        v.extend_from_slice(&self.local_capacity.mem_mib.to_le_bytes());
        v.extend_from_slice(&self.local_capacity.net_score.to_le_bytes());
        v
    }

    pub fn try_parse_heartbeat(buf: &[u8]) -> Option<(u32, bool)> {
        Self::try_parse_heartbeat_full(buf).map(|(id, pk, _)| (id, pk))
    }

    /// `CAP\0` + 3×u32 LE após `PK\0` quando presente (eleição por capacity real).
    pub fn try_parse_heartbeat_full(buf: &[u8]) -> Option<(u32, bool, Option<CapacityScore>)> {
        if buf.len() < 7 || &buf[0..3] != MAGIC_HB {
            return None;
        }
        let id = u32::from_le_bytes(buf[3..7].try_into().ok()?);
        let has_pk = buf.windows(3).any(|w| w == b"PK\0");
        let cap = parse_hb_capacity(buf);
        Some((id, has_pk, cap))
    }

    /// ROLE\0{target_le32}{role_u8} — None se TOFU ainda não settle.
    pub fn try_encode_role(&self, target: u32, role: PeerRole) -> Option<Vec<u8>> {
        if !self.may_send_control_sync() {
            return None;
        }
        Some(self.encode_role_unchecked(target, role))
    }

    pub fn encode_role_unchecked(&self, target: u32, role: PeerRole) -> Vec<u8> {
        let role_u8 = match role {
            PeerRole::Master => 0,
            PeerRole::Worker => 1,
            PeerRole::Memory => 2,
            PeerRole::Compute => 3,
        };
        let mut v = MAGIC_ROLE.to_vec();
        v.extend_from_slice(&target.to_le_bytes());
        v.push(role_u8);
        v
    }

    pub fn try_parse_role(buf: &[u8]) -> Option<(u32, PeerRole)> {
        if buf.len() < 10 || &buf[0..5] != MAGIC_ROLE {
            return None;
        }
        let target = u32::from_le_bytes(buf[5..9].try_into().ok()?);
        let role = match buf[9] {
            0 => PeerRole::Master,
            2 => PeerRole::Memory,
            3 => PeerRole::Compute,
            _ => PeerRole::Worker,
        };
        Some((target, role))
    }

    pub fn encode_frack(&self, stream_id: u32, seq: u32) -> Vec<u8> {
        Frack { stream_id, seq }.encode()
    }

    pub fn take_pending_fracks(&self) -> Vec<Frack> {
        std::mem::take(&mut *self.pending_fracks.lock())
    }

    pub fn fracks_received(&self) -> Vec<Frack> {
        self.fracks_received.lock().clone()
    }

    /// Processa datagrama: HB, FRACK ou FRAG. Retorna Some(payload) se FRAG completo.
    pub fn on_datagram(&self, buf: &[u8]) -> Option<Vec<u8>> {
        if let Some(h) = parse_mch(buf) {
            let peer = u32::from(h.node_id);
            if peer != self.local_id {
                self.peer_fleet.lock().insert(peer, h);
            }
            return None;
        }
        if let Some(li) = LayaIntent::decode(buf) {
            if li.node_id != self.local_id {
                info!(
                    target: "nosd",
                    "mesh LayaIntent peer={} arm={} fnv={:016x}",
                    li.node_id,
                    li.intent_arm,
                    li.text_fnv
                );
                *self.last_laya.lock() = Some(li);
            }
            return None;
        }
        if let Some((id, pk, cap)) = Self::try_parse_heartbeat_full(buf) {
            if id != self.local_id {
                self.on_heartbeat_inner(id, pk, cap);
            }
            return None;
        }
        if let Some((target, role)) = Self::try_parse_role(buf) {
            if target == self.local_id {
                *self.local_role.lock() = role;
            }
            return None;
        }
        if let Some(ack) = Frack::parse(buf) {
            self.fracks_received.lock().push(ack);
            info!(
                target: "nosd",
                "mesh FRACK stream={} seq={}",
                ack.stream_id,
                ack.seq
            );
            return None;
        }
        if let Some((h, _payload)) = FragReassembler::parse(buf) {
            self.pending_fracks
                .lock()
                .push(Frack::from_frag(h.msg_id, h.frag_idx));
        }
        if let Some(payload) = self.frags.push(buf) {
            info!(
                target: "nosd",
                "mesh FRAG complete bytes={}",
                payload.len()
            );
            *self.last_frag.lock() = Some(payload.clone());
            return Some(payload);
        }
        None
    }

    pub fn split_frag(&self, msg_id: u32, payload: &[u8], chunk: usize) -> Vec<Vec<u8>> {
        FragReassembler::split(msg_id, payload, chunk)
    }

    pub fn last_frag_len(&self) -> usize {
        self.last_frag.lock().as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn health_json(&self) -> String {
        format!(
            r#"[{{"local":{},"peers":{},"tofu":{},"last_frag":{},"role":"{:?}","pending_fracks":{}}}]"#,
            self.local_id,
            self.peer_count(),
            self.tofu_settled() as u8,
            self.last_frag_len(),
            self.local_role(),
            self.pending_fracks.lock().len()
        )
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_hb_capacity(buf: &[u8]) -> Option<CapacityScore> {
    let pk_off = buf.windows(3).position(|w| w == b"PK\0")?;
    let mut off = pk_off + 3;
    if off + 4 > buf.len() || &buf[off..off + 4] != b"CAP\0" {
        return None;
    }
    off += 4;
    if off + 12 > buf.len() {
        return None;
    }
    let cpu = u32::from_le_bytes(buf[off..off + 4].try_into().ok()?);
    off += 4;
    let mem = u32::from_le_bytes(buf[off..off + 4].try_into().ok()?);
    off += 4;
    let net = u32::from_le_bytes(buf[off..off + 4].try_into().ok()?);
    Some(CapacityScore {
        cpu_units: cpu,
        mem_mib: mem,
        net_score: net,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hb_roundtrip_with_capacity() {
        let m = MeshEngine::with_capacity(
            15,
            CapacityScore {
                cpu_units: 8,
                mem_mib: 4096,
                net_score: 200,
            },
        );
        let buf = m.encode_heartbeat();
        let (id, pk, cap) = MeshEngine::try_parse_heartbeat_full(&buf).unwrap();
        assert_eq!(id, 15);
        assert!(pk);
        let cap = cap.unwrap();
        assert_eq!(cap.cpu_units, 8);
        assert_eq!(cap.mem_mib, 4096);
        m.on_heartbeat(2, true);
        assert_eq!(m.peer_count(), 1);
        assert!(!m.tofu_settled());
    }

    #[test]
    fn tofu_settled_after_ticks() {
        let m = MeshEngine::new(1);
        m.on_heartbeat(2, true);
        assert_eq!(m.peer_tofu_count(), 1);
        assert!(!m.tofu_settled());
        for _ in 0..TOFU_SETTLE_TICKS {
            m.tick();
        }
        assert!(m.tofu_settled());
    }

    #[test]
    fn role_encode_gated_until_tofu() {
        let m = MeshEngine::new(1);
        assert!(m.try_encode_role(1, PeerRole::Master).is_none());
        m.on_heartbeat(2, true);
        for _ in 0..TOFU_SETTLE_TICKS {
            m.tick();
        }
        let role = m.try_encode_role(1, PeerRole::Master).unwrap();
        assert_eq!(&role[0..5], MAGIC_ROLE);
    }

    #[test]
    fn frack_encode_wire() {
        let m = MeshEngine::new(1);
        let buf = m.encode_frack(42, 7);
        assert_eq!(buf.len(), frack::HDR_LEN);
        let parsed = Frack::parse(&buf).unwrap();
        assert_eq!(parsed.stream_id, 42);
        assert_eq!(parsed.seq, 7);
    }

    #[test]
    fn election_uses_hb_capacity() {
        let local = MeshEngine::with_capacity(
            10,
            CapacityScore {
                cpu_units: 32,
                mem_mib: 8192,
                net_score: 100,
            },
        );
        let peer_hb = MeshEngine::with_capacity(
            20,
            CapacityScore {
                cpu_units: 4,
                mem_mib: 1024,
                net_score: 50,
            },
        );
        let buf = peer_hb.encode_heartbeat();
        local.on_datagram(&buf);
        assert_eq!(local.local_role(), PeerRole::Master);
        assert_eq!(local.peer_role(20), Some(PeerRole::Worker));
    }

    #[test]
    fn frag_via_engine() {
        let m = MeshEngine::new(1);
        let parts = m.split_frag(7, b"abcdEFGH", 3);
        let mut n = 0;
        for p in parts {
            if m.on_datagram(&p).is_some() {
                n += 1;
            }
        }
        assert_eq!(n, 1);
        assert_eq!(m.last_frag_len(), 8);
    }

    #[test]
    fn frack_on_partial_frag() {
        let m = MeshEngine::new(1);
        let parts = m.split_frag(5, b"0123456789", 4);
        assert!(parts.len() > 1);
        let _ = m.on_datagram(&parts[0]);
        let pending = m.take_pending_fracks();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].stream_id, 5);
        assert_eq!(pending[0].seq, 0);
        let ack = m.encode_frack(5, 0);
        m.on_datagram(&ack);
        assert_eq!(m.fracks_received().len(), 1);
    }

    #[test]
    fn election_highest_capacity_master() {
        let local = MeshEngine::with_capacity(
            10,
            CapacityScore {
                cpu_units: 2,
                mem_mib: 512,
                net_score: 10,
            },
        );
        local.on_heartbeat_with_capacity(
            20,
            true,
            CapacityScore {
                cpu_units: 16,
                mem_mib: 8192,
                net_score: 500,
            },
        );
        assert_eq!(local.local_role(), PeerRole::Worker);
        assert_eq!(local.peer_role(20), Some(PeerRole::Master));
    }

    #[test]
    fn laya_intent_datagram() {
        let m = MeshEngine::new(1);
        let li = LayaIntent {
            node_id: 2,
            clock: 1,
            intent_arm: 4,
            confidence_q8: 180,
            margin_q8: 20,
            text_fnv: content_fingerprint("mesh lab"),
        };
        m.on_datagram(&li.encode());
        let got = m.last_laya_intent().unwrap();
        assert_eq!(got.node_id, 2);
        assert_eq!(got.intent_arm, 4);
    }

    #[test]
    fn tick_extras_mch_and_laya() {
        let m = MeshEngine::new(7);
        m.set_local_fleet_health(MachineHealth {
            node_id: 7,
            sys: HealthVerdict::Go,
            audio: HealthVerdict::Unknown,
            hub: HealthVerdict::Go,
            reason: "test".into(),
        });
        m.queue_laya_intent(LayaIntent {
            node_id: 7,
            clock: 3,
            intent_arm: 1,
            confidence_q8: 200,
            margin_q8: 10,
            text_fnv: 0xabc,
        }); // types: node_id u32, clock u64
        let extras = m.encode_tick_extras();
        assert_eq!(extras.len(), 2);
        assert!(parse_mch(&extras[0]).is_some());
        assert!(LayaIntent::decode(&extras[1]).is_some());
    }

    #[test]
    fn local_wins_election() {
        let m = MeshEngine::with_capacity(
            1,
            CapacityScore {
                cpu_units: 64,
                mem_mib: 32_768,
                net_score: 1000,
            },
        );
        m.on_heartbeat_with_capacity(
            2,
            true,
            CapacityScore {
                cpu_units: 1,
                mem_mib: 256,
                net_score: 1,
            },
        );
        assert_eq!(m.local_role(), PeerRole::Master);
    }

    /// M5/ADR-0081: o settle do TOFU (janela de 7.2s) destranca ROLE/SkillSync.
    #[test]
    fn tofu_settles_after_window() {
        let m = MeshEngine::new(1);
        assert!(!m.tofu_settled(), "sem peers nao ha settle");
        m.on_heartbeat(2, true);
        assert_eq!(m.peer_tofu_count(), 1, "HB com PK -> peer pk-bound");
        assert!(!m.tofu_settled(), "recem-ligado ainda nao settle");
        std::thread::sleep(std::time::Duration::from_millis(TOFU_SETTLE_MS + 300));
        assert!(m.tofu_settled(), "deve settle ao fim de TOFU_SETTLE_MS");
    }
}
