//! M6.1 — canal cifrado (Noise XX) + binding da identidade Ed25519.
//!
//! O Noise dá sigilo + forward secrecy com estáticas X25519 **descartáveis**
//! (geradas por ligação). A identidade continua a ser a chave Ed25519 do mesh:
//! depois do handshake cada lado envia `BIND`, assinado sobre
//! `"NJOBBIND" || handshake_hash || pubkey || node_id`, e o recetor exige:
//!   - assinatura válida,
//!   - `id == short_id(pubkey)`,
//!   - chave **fixada** (TOFU/allowlist) no store partilhado com o mesh.
//!
//! Não há conversão Ed25519→X25519 nem PKI: o Noise protege, o Ed25519
//! autentica (separação canal/identidade, como no libp2p).
//!
//! Framing: dentro do Noise, `len u32 LE | bytes`; plaintext máximo por
//! mensagem Noise = 65519 B, por isso `send`/`recv` fragmentam.

use crate::ident::{short_id, Identity, Pinned};
use snow::params::NoiseParams;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::info;

const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// plaintext máximo por mensagem de transporte (65535 - 16 tag).
const MAX_CHUNK: usize = 65_519;
pub const BIND_LABEL: &[u8] = b"NJOBBIND";

fn params() -> NoiseParams {
    NOISE_PARAMS.parse().expect("Noise params estáticos")
}

async fn write_frame(s: &mut TcpStream, b: &[u8]) -> Result<(), String> {
    let n = u32::try_from(b.len()).map_err(|_| "frame grande".to_string())?;
    s.write_all(&n.to_le_bytes())
        .await
        .map_err(|e| e.to_string())?;
    s.write_all(b).await.map_err(|e| e.to_string())
}

async fn read_frame(s: &mut TcpStream, max: usize) -> Result<Vec<u8>, String> {
    let mut h = [0u8; 4];
    s.read_exact(&mut h).await.map_err(|e| e.to_string())?;
    let n = u32::from_le_bytes(h) as usize;
    if n > max {
        return Err(format!("frame {n} acima do limite"));
    }
    let mut b = vec![0u8; n];
    s.read_exact(&mut b).await.map_err(|e| e.to_string())?;
    Ok(b)
}

pub struct Secure {
    sock: TcpStream,
    tx: snow::TransportState,
    peer: u32,
    peer_key: [u8; 32],
    hash: Vec<u8>,
}

impl Secure {
    pub async fn initiator(
        mut sock: TcpStream,
        identity: &Identity,
        pinned: &Arc<Pinned>,
    ) -> Result<Self, String> {
        let b = snow::Builder::new(params());
        let kp = b.generate_keypair().map_err(|e| e.to_string())?;
        let mut hs = b
            .local_private_key(&kp.private)
            .build_initiator()
            .map_err(|e| e.to_string())?;

        let mut buf = vec![0u8; 65_535];
        let n = hs.write_message(&[], &mut buf).map_err(|e| e.to_string())?;
        write_frame(&mut sock, &buf[..n]).await?;

        let m = read_frame(&mut sock, 65_535).await?;
        hs.read_message(&m, &mut buf).map_err(|e| e.to_string())?;

        let n = hs.write_message(&[], &mut buf).map_err(|e| e.to_string())?;
        write_frame(&mut sock, &buf[..n]).await?;

        let hash = hs.get_handshake_hash().to_vec();
        let tx = hs.into_transport_mode().map_err(|e| e.to_string())?;
        let mut s = Self {
            sock,
            tx,
            peer: 0,
            peer_key: [0u8; 32],
            hash,
        };
        s.send_bind(identity).await?;
        let (peer, key) = s.recv_bind().await?;
        pinned
            .check_key(peer, &key)
            .map_err(|e| format!("peer nao autorizado: {e:?}"))?;
        s.peer = peer;
        s.peer_key = key;
        info!(target: "nosd", "noise initiator peer={peer} cifrado");
        Ok(s)
    }

    pub async fn responder(
        mut sock: TcpStream,
        identity: &Identity,
        pinned: &Arc<Pinned>,
    ) -> Result<Self, String> {
        let b = snow::Builder::new(params());
        let kp = b.generate_keypair().map_err(|e| e.to_string())?;
        let mut hs = b
            .local_private_key(&kp.private)
            .build_responder()
            .map_err(|e| e.to_string())?;

        let mut buf = vec![0u8; 65_535];
        let m = read_frame(&mut sock, 65_535).await?;
        hs.read_message(&m, &mut buf).map_err(|e| e.to_string())?;

        let n = hs.write_message(&[], &mut buf).map_err(|e| e.to_string())?;
        write_frame(&mut sock, &buf[..n]).await?;

        let m = read_frame(&mut sock, 65_535).await?;
        hs.read_message(&m, &mut buf).map_err(|e| e.to_string())?;

        let hash = hs.get_handshake_hash().to_vec();
        let tx = hs.into_transport_mode().map_err(|e| e.to_string())?;
        let mut s = Self {
            sock,
            tx,
            peer: 0,
            peer_key: [0u8; 32],
            hash,
        };
        let (peer, key) = s.recv_bind().await?;
        pinned
            .check_key(peer, &key)
            .map_err(|e| format!("peer nao autorizado: {e:?}"))?;
        s.peer = peer;
        s.peer_key = key;
        s.send_bind(identity).await?;
        info!(target: "nosd", "noise responder peer={peer} cifrado");
        Ok(s)
    }

    pub fn peer(&self) -> u32 {
        self.peer
    }

    pub fn peer_key(&self) -> [u8; 32] {
        self.peer_key
    }

    fn bind_payload(&self, identity: &Identity) -> Vec<u8> {
        let pk = identity.public_key();
        let id = identity.node_id();
        let mut msg = BIND_LABEL.to_vec();
        msg.extend_from_slice(&self.hash);
        msg.extend_from_slice(&pk);
        msg.extend_from_slice(&id.to_le_bytes());
        let sig = identity.sign(&msg);
        let mut out = Vec::with_capacity(100);
        out.extend_from_slice(&pk);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&sig);
        out
    }

    async fn send_bind(&mut self, identity: &Identity) -> Result<(), String> {
        let payload = self.bind_payload(identity);
        self.send(&payload).await
    }

    async fn recv_bind(&mut self) -> Result<(u32, [u8; 32]), String> {
        let b = self.recv().await?;
        if b.len() != 100 {
            return Err("BIND de tamanho invalido".into());
        }
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&b[..32]);
        let id = u32::from_le_bytes(b[32..36].try_into().unwrap());
        let sig = &b[36..100];
        if short_id(&pk) != id {
            return Err("BIND: id nao corresponde a chave".into());
        }
        let mut msg = BIND_LABEL.to_vec();
        msg.extend_from_slice(&self.hash);
        msg.extend_from_slice(&pk);
        msg.extend_from_slice(&id.to_le_bytes());
        if !crate::ident::verify(&pk, &msg, sig) {
            return Err("BIND: assinatura invalida".into());
        }
        Ok((id, pk))
    }

    /// Envia um payload (fragmentado em mensagens Noise).
    pub async fn send(&mut self, plain: &[u8]) -> Result<(), String> {
        let mut buf = vec![0u8; 65_535];
        let n = self
            .tx
            .write_message(&(plain.len() as u32).to_le_bytes(), &mut buf)
            .map_err(|e| e.to_string())?;
        write_frame(&mut self.sock, &buf[..n]).await?;
        for chunk in plain.chunks(MAX_CHUNK) {
            let n = self
                .tx
                .write_message(chunk, &mut buf)
                .map_err(|e| e.to_string())?;
            write_frame(&mut self.sock, &buf[..n]).await?;
        }
        Ok(())
    }

    /// Recebe um payload (reassembla as mensagens Noise).
    pub async fn recv(&mut self) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; 65_535];
        let h = read_frame(&mut self.sock, 65_535).await?;
        let mut head = [0u8; 4];
        let n = self
            .tx
            .read_message(&h, &mut buf)
            .map_err(|e| e.to_string())?;
        if n != 4 {
            return Err("cabeçalho de tamanho invalido".into());
        }
        head.copy_from_slice(&buf[..4]);
        let total = u32::from_le_bytes(head) as usize;
        if total > 1 << 20 {
            return Err("payload acima de 1 MiB".into());
        }
        let mut out = Vec::with_capacity(total);
        while out.len() < total {
            let m = read_frame(&mut self.sock, 65_535).await?;
            let n = self
                .tx
                .read_message(&m, &mut buf)
                .map_err(|e| e.to_string())?;
            out.extend_from_slice(&buf[..n]);
        }
        Ok(out)
    }
}
