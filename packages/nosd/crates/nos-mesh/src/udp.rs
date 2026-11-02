//! Transporte UDP mesh (HB + FRAG reassembly + FRACK TX após RX FRAG).

use crate::MeshEngine;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::{info, warn};

/// Bind UDP + loop RX; TX heartbeat periódico via `tick_tx`.
pub struct MeshUdp {
    sock: Arc<UdpSocket>,
    mesh: MeshEngine,
    broadcast: SocketAddr,
    /// Broadcast + unicast seeds, deduped (see `destinations`): all TX goes to
    /// every target so a dead broadcast route does not silence the fleet.
    targets: Vec<SocketAddr>,
    /// Contador de ENETUNREACH consecutivos — reduz spam até a iface (Wi‑Fi/eth) subir.
    unreachable_streak: AtomicU64,
}

/// TX target set: broadcast + unicast seeds, deduped.
///
/// Fail-closed: a seed that does not parse aborts startup instead of being
/// silently skipped (a typo hidden behind a working broadcast is a fleet that
/// nobody sees).
fn destinations(broadcast: SocketAddr, peers: &[String]) -> std::io::Result<Vec<SocketAddr>> {
    let mut out = Vec::with_capacity(peers.len() + 1);
    out.push(broadcast);
    for raw in peers {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        let addr: SocketAddr = s.parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("mesh-peer invalido ({s:?}): {e}"),
            )
        })?;
        if !out.contains(&addr) {
            out.push(addr);
        }
    }
    Ok(out)
}

impl MeshUdp {
    pub async fn bind(
        bind: &str,
        broadcast: &str,
        peers: &[String],
        mesh: MeshEngine,
    ) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(bind).await?;
        sock.set_broadcast(true)?;
        let broadcast: SocketAddr = broadcast
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let targets = destinations(broadcast, peers)?;
        info!(
            target: "nosd",
            "mesh udp bind={} broadcast={} targets={} peers={}",
            bind,
            broadcast,
            targets.len(),
            targets.len().saturating_sub(1)
        );
        Ok(Self {
            sock: Arc::new(sock),
            mesh,
            broadcast,
            targets,
            unreachable_streak: AtomicU64::new(0),
        })
    }

    /// Configured TX addresses (broadcast first).
    pub fn targets(&self) -> &[SocketAddr] {
        &self.targets
    }

    /// Send `buf` to every target; one success clears the unreachable streak.
    /// Returns `true` if at least one destination accepted the datagram.
    async fn tx_all(&self, buf: &[u8], kind: &str) -> bool {
        let mut ok_any = false;
        let mut last_err = None;
        for target in &self.targets {
            match self.sock.send_to(buf, *target).await {
                Ok(_) => ok_any = true,
                Err(e) => last_err = Some(e),
            }
        }
        if ok_any {
            self.unreachable_streak.store(0, Ordering::Relaxed);
        } else if let Some(e) = last_err {
            self.note_tx_err(&e, kind);
        }
        ok_any
    }

    fn note_tx_err(&self, e: &std::io::Error, kind: &str) {
        let unreachable = e.raw_os_error() == Some(101) // ENETUNREACH
            || e.kind() == std::io::ErrorKind::NetworkUnreachable;
        if unreachable {
            let n = self.unreachable_streak.fetch_add(1, Ordering::Relaxed) + 1;
            // 1º, depois a cada 32 ticks (~1 min com tick 2s) até a rede (Wi‑Fi notes / eth AMD) subir.
            if n == 1 || n.is_multiple_of(32) {
                warn!(
                    target: "nosd",
                    "mesh {kind} ENETUNREACH x{n} (aguarda Wi‑Fi/eth na mesma LAN; broadcast={})",
                    self.broadcast
                );
            }
        } else {
            self.unreachable_streak.store(0, Ordering::Relaxed);
            warn!(target: "nosd", "mesh {kind} err={e}");
        }
    }

    pub fn mesh(&self) -> &MeshEngine {
        &self.mesh
    }

    pub fn sock(&self) -> Arc<UdpSocket> {
        self.sock.clone()
    }

    /// Envia um heartbeat + MCH/LayaIntent extras (M.2/M.3 residual).
    pub async fn tick_tx(&self) {
        self.mesh.tick();
        let buf = self.mesh.encode_heartbeat();
        let _ = self.tx_all(&buf, "HB tx").await;
        for extra in self.mesh.encode_tick_extras() {
            let _ = self.tx_all(&extra, "tick extra tx").await;
        }
    }

    async fn flush_pending_fracks(&self) {
        for ack in self.mesh.take_pending_fracks() {
            let buf = ack.encode();
            let _ = self.tx_all(&buf, "FRACK tx").await;
        }
    }

    /// Envia payload fragmentado (lab SkillSync / knowledge) — gated TOFU settle.
    pub async fn send_fragmented(&self, msg_id: u32, payload: &[u8]) {
        if !self.mesh.may_send_control_sync() {
            warn!(target: "nosd", "mesh SkillSync skip tofu unsettled");
            return;
        }
        for part in self.mesh.split_frag(msg_id, payload, 1024) {
            if !self.tx_all(&part, "FRAG tx").await {
                break;
            }
        }
    }

    /// Drena até `max` datagramas (non-blocking via try_recv patten with timeout 0).
    pub async fn poll_rx(&self, max: usize) {
        let mut buf = [0u8; 1500];
        for _ in 0..max {
            match self.sock.try_recv_from(&mut buf) {
                Ok((n, _src)) => {
                    let _ = self.mesh.on_datagram(&buf[..n]);
                    self.flush_pending_fracks().await;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    warn!(target: "nosd", "mesh rx err={e}");
                    break;
                }
            }
        }
    }
}

/// Spawn RX task dedicada (recv bloqueante) + devolve handle TX.
pub async fn start_mesh_udp(
    bind: &str,
    broadcast: &str,
    peers: &[String],
    mesh: MeshEngine,
) -> std::io::Result<Arc<MeshUdp>> {
    let udp = Arc::new(MeshUdp::bind(bind, broadcast, peers, mesh).await?);
    let rx = udp.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            match rx.sock().recv_from(&mut buf).await {
                Ok((n, _src)) => {
                    let _ = rx.mesh().on_datagram(&buf[..n]);
                    rx.flush_pending_fracks().await;
                }
                Err(e) => {
                    warn!(target: "nosd", "mesh recv loop err={e}");
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
    });
    Ok(udp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_dedup_and_skip_blank() {
        let b: SocketAddr = "127.0.0.1:19000".parse().unwrap();
        let out = destinations(
            b,
            &[
                "127.0.0.1:19000".into(), // same as broadcast: dedup
                "10.0.0.7:19000".into(),
                "10.0.0.7:19000".into(),
                "   ".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            out,
            vec![b, "10.0.0.7:19000".parse().unwrap()],
            "broadcast first, seeds deduped, blanks ignored"
        );
    }

    #[test]
    fn destinations_reject_invalid_seed() {
        let b: SocketAddr = "255.255.255.255:19000".parse().unwrap();
        for bad in ["10.0.0.7", "not-an-ip:19000", "10.0.0.7:not-a-port"] {
            let err = destinations(b, &[bad.into()]).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "{bad}");
        }
    }

    #[test]
    fn destinations_accept_ipv6_literal() {
        let b: SocketAddr = "127.0.0.1:19000".parse().unwrap();
        let out = destinations(b, &["[::1]:19000".into()]).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out[1].is_ipv6());
    }

    #[tokio::test]
    async fn unicast_seed_delivers_hb_without_broadcast() {
        // B on an ephemeral port. A points the seed at B and the broadcast at
        // a closed port, so the only working path is the seed.
        let b = MeshUdp::bind("127.0.0.1:0", "127.0.0.1:1", &[], MeshEngine::new(2))
            .await
            .unwrap();
        let b_addr = b.sock().local_addr().unwrap();
        let a = MeshUdp::bind(
            "127.0.0.1:0",
            "127.0.0.1:1",
            &[b_addr.to_string()],
            MeshEngine::new(1),
        )
        .await
        .unwrap();
        assert_eq!(a.targets().len(), 2, "broadcast + one seed");

        a.tick_tx().await;
        let mut buf = [0u8; 1500];
        let (n, _) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            b.sock().recv_from(&mut buf),
        )
        .await
        .expect("HB did not arrive through the unicast seed")
        .unwrap();
        assert!(n > 0);
        b.mesh().on_datagram(&buf[..n]);
        assert_eq!(b.mesh().peer_count(), 1, "B must see A via the seed");
    }
}
