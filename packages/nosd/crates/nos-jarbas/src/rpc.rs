//! Cliente IPC nosd (Unix socket / TCP) — partilhado HUD slog + Slint.

use nos_ipc::{decode_body, encode, take_frame, Request, Response};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub enum Endpoint {
    #[cfg(unix)]
    Unix(String),
    Tcp {
        addr: String,
        token: Option<String>,
    },
}

impl Endpoint {
    pub fn from_parts(socket: &str, tcp: Option<&str>, token: Option<String>) -> Self {
        if let Some(addr) = tcp {
            return Self::Tcp {
                addr: addr.to_string(),
                token,
            };
        }
        #[cfg(unix)]
        {
            let _ = token;
            Self::Unix(socket.to_string())
        }
        #[cfg(not(unix))]
        {
            let _ = socket;
            Self::Tcp {
                addr: "127.0.0.1:7700".into(),
                token,
            }
        }
    }

    fn token(&self) -> Option<&str> {
        match self {
            #[cfg(unix)]
            Self::Unix(_) => None,
            Self::Tcp { token, .. } => token.as_deref(),
        }
    }

    pub async fn rpc(&self, mut req: Request) -> anyhow::Result<Response> {
        if let Some(token) = self.token() {
            req.token = Some(token.to_string());
        }
        let enc = encode(&req)?;
        match self {
            #[cfg(unix)]
            Self::Unix(path) => {
                let mut s = tokio::net::UnixStream::connect(path).await?;
                s.write_all(&enc).await?;
                s.shutdown().await?;
                read_resp(&mut s).await
            }
            Self::Tcp { addr, .. } => {
                let mut s = tokio::net::TcpStream::connect(addr).await?;
                s.write_all(&enc).await?;
                s.shutdown().await?;
                read_resp(&mut s).await
            }
        }
    }

    /// Sync wrapper for Slint UI thread (blocks briefly).
    #[cfg(feature = "slint-ui")]
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
        agent: "jarbas".into(), // legacy hint; never auth
        token: None,            // Endpoint::rpc stamps for TCP
        params,
    };
    *id = id.wrapping_add(1);
    req
}

pub fn load_token(explicit: Option<String>, file: Option<PathBuf>) -> Option<String> {
    if let Some(token) = explicit {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(path) = file {
        paths.push(path);
    }
    if let Ok(path) = std::env::var("NOSD_IPC_TOKEN_FILE") {
        paths.push(PathBuf::from(path));
    }
    paths.push(PathBuf::from("/run/nosd/ipc.token"));
    paths.push(PathBuf::from("/var/lib/nosd/ipc.token"));
    paths.push(PathBuf::from("data/ipc.token"));
    for path in paths {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let token = text.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}
