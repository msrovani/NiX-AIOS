//! Orb lab HTTP — fatia 0.5.2.
use super::*;
use nos_ipc::Request;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn};

const ORB_LAB_HTML: &str = include_str!("../../static/orb_lab.html");
const TRANSMUTE_HTML: &str = include_str!("../../static/transmute.html");

/// Lab cinemático: GET `/` ? HTML (cam+mic+padrões); GET `/transmute` ? wizard conversão; POST `/rpc` ? dispatch IPC.
pub async fn serve_orb_lab(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(target: "nosd", "orb lab listening http://{addr}");
    loop {
        if state.shutting_down.load(Ordering::Relaxed) {
            break;
        }
        let (mut sock, _) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_orb_lab_conn(&mut sock, st).await {
                warn!(target: "nosd", "orb lab conn: {e}");
            }
        });
    }
    Ok(())
}

async fn handle_orb_lab_conn<S>(sock: &mut S, state: Arc<AppState>) -> anyhow::Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buf = vec![0u8; 65536];
    let n = sock.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    let first = head.lines().next().unwrap_or("");
    let is_post_rpc = first.starts_with("POST /rpc");
    let is_transmute = first.starts_with("GET /transmute");
    let is_get = first.starts_with("GET / ") || first.starts_with("GET /index") || first.starts_with("GET /?");

    if is_transmute {
        let body = TRANSMUTE_HTML.as_bytes();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
            body.len()
        );
        sock.write_all(resp.as_bytes()).await?;
        sock.write_all(body).await?;
        return Ok(());
    }

    if is_get {
        let body = ORB_LAB_HTML.as_bytes();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
            body.len()
        );
        sock.write_all(resp.as_bytes()).await?;
        sock.write_all(body).await?;
        return Ok(());
    }

    if first.starts_with("OPTIONS ") {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
        sock.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    if !is_post_rpc {
        let msg = b"not found";
        let resp = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            msg.len()
        );
        sock.write_all(resp.as_bytes()).await?;
        sock.write_all(msg).await?;
        return Ok(());
    }

    // Body: may already be in buf after headers
    let header_end = head
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| head.find("\n\n").map(|i| i + 2))
        .unwrap_or(n);
    let mut body = buf[header_end.min(n)..n].to_vec();
    let content_len = head
        .lines()
        .find_map(|l| {
            let l = l.to_ascii_lowercase();
            l.strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    while body.len() < content_len {
        let m = sock.read(&mut buf).await?;
        if m == 0 {
            break;
        }
        body.extend_from_slice(&buf[..m]);
    }
    body.truncate(content_len);

    let req: Request = serde_json::from_slice(&body).unwrap_or(Request {
        id: 0,
        method: "health".into(),
        caps: 0,
        cap_gen: 1,
        agent: "nosctl".into(),
        params: json!({}),
    });
    let resp = super::dispatch(&state, req);
    let payload = serde_json::to_vec(&resp)?;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        payload.len()
    );
    sock.write_all(header.as_bytes()).await?;
    sock.write_all(&payload).await?;
    Ok(())
}
