//! Cliente IPC nosd (Unix socket / TCP) — partilhado HUD slog + Slint.

use nos_ipc::{Request, Response, decode_body, encode, take_frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub enum Endpoint {
    #[cfg(unix)]
    Unix(String),
    Tcp(String),
}

impl Endpoint {
    pub fn from_parts(socket: &str, tcp: Option<&str>) -> Self {
        if let Some(addr) = tcp {
            return Self::Tcp(addr.to_string());
        }
        #[cfg(unix)]
        {
            Self::Unix(socket.to_string())
        }
        #[cfg(not(unix))]
        {
            let _ = socket;
            Self::Tcp("127.0.0.1:7700".into())
        }
    }

    pub async fn rpc(&self, req: Request) -> anyhow::Result<Response> {
        let enc = encode(&req)?;
        match self {
            #[cfg(unix)]
            Self::Unix(path) => {
                let mut s = tokio::net::UnixStream::connect(path).await?;
                s.write_all(&enc).await?;
                s.shutdown().await?;
                read_resp(&mut s).await
            }
            Self::Tcp(addr) => {
                let mut s = tokio::net::TcpStream::connect(addr).await?;
                s.write_all(&enc).await?;
                s.shutdown().await?;
                read_resp(&mut s).await
            }
        }
    }

    /// Sync wrapper for Slint UI thread (blocks briefly).
    pub fn rpc_blocking(&self, req: Request) -> anyhow::Result<Response> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.rpc(req))
    }
}

async fn read_resp<S: AsyncReadExt + Unpin>(s: &mut S) -> anyhow::Result<Response> {
    let mut acc = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = s.read(&mut buf).await?;
        if n == 0 {
            anyhow::bail!("eof before frame");
        }
        acc.extend_from_slice(&buf[..n]);
        if let Some((body, _)) = take_frame(&acc)? {
            return Ok(decode_body(&body)?);
        }
    }
}

pub fn next_req(id: &mut u64, method: &str, caps: u32, params: serde_json::Value) -> Request {
    let req = Request {
        id: *id,
        method: method.into(),
        caps, // ignored server-side (I.1)
        cap_gen: 1,
        agent: "jarbas".into(),
        params,
    };
    *id = id.wrapping_add(1);
    req
}
