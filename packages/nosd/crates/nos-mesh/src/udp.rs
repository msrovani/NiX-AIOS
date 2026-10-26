//! Transporte UDP mesh (HB + FRAG reassembly + FRACK TX após RX FRAG).

use crate::MeshEngine;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::{info, warn};

/// Bind UDP + loop RX; TX heartbeat periódico via `tick_tx`.
pub struct MeshUdp {
    sock: Arc<UdpSocket>,
    mesh: MeshEngine,
    broadcast: SocketAddr,
}

impl MeshUdp {
    pub async fn bind(bind: &str, broadcast: &str, mesh: MeshEngine) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(bind).await?;
        sock.set_broadcast(true)?;
        let broadcast: SocketAddr = broadcast
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        info!(target: "nosd", "mesh udp bind={} broadcast={}", bind, broadcast);
        Ok(Self {
            sock: Arc::new(sock),
            mesh,
            broadcast,
        })
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
        if let Err(e) = self.sock.send_to(&buf, self.broadcast).await {
            warn!(target: "nosd", "mesh HB tx err={e}");
        }
        for extra in self.mesh.encode_tick_extras() {
            if let Err(e) = self.sock.send_to(&extra, self.broadcast).await {
                warn!(target: "nosd", "mesh tick extra tx err={e}");
            }
        }
    }

    async fn flush_pending_fracks(&self) {
        for ack in self.mesh.take_pending_fracks() {
            let buf = ack.encode();
            if let Err(e) = self.sock.send_to(&buf, self.broadcast).await {
                warn!(target: "nosd", "mesh FRACK tx err={e}");
            }
        }
    }

    /// Envia payload fragmentado (lab SkillSync / knowledge) — gated TOFU settle.
    pub async fn send_fragmented(&self, msg_id: u32, payload: &[u8]) {
        if !self.mesh.may_send_control_sync() {
            warn!(target: "nosd", "mesh SkillSync skip tofu unsettled");
            return;
        }
        for part in self.mesh.split_frag(msg_id, payload, 1024) {
            if let Err(e) = self.sock.send_to(&part, self.broadcast).await {
                warn!(target: "nosd", "mesh FRAG tx err={e}");
                break;
            }
        }
    }

    /// Drena até `max` datagramas (non-blocking via try_recv pattern with timeout 0).
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
    mesh: MeshEngine,
) -> std::io::Result<Arc<MeshUdp>> {
    let udp = Arc::new(MeshUdp::bind(bind, broadcast, mesh).await?);
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
