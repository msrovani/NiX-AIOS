//! Orb lab HTTP — fatia 0.5.4.
//!
//! Segurança: `/rpc` exige token de processo, JSON, same-origin (quando há
//! `Origin`) e limites de corpo/tempo. Sem `Access-Control-Allow-Origin: *`.
use super::*;
use nos_ipc::Request;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn};

const ORB_LAB_HTML: &str = include_str!("../../static/orb_lab.html");
const TRANSMUTE_HTML: &str = include_str!("../../static/transmute.html");
const MAX_BODY: usize = 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Lab cinemático: GET `/` → HTML (cam+mic+padrões); GET `/transmute` → wizard;
/// POST `/rpc` → dispatch autenticado por token.
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
    let is_get = first.starts_with("GET / ")
        || first.starts_with("GET /index")
        || first.starts_with("GET /?");

    let token = state.ipc_token();

    if is_transmute {
        let body = TRANSMUTE_HTML.replace("__NOSD_TOKEN__", &token);
        return write_response(sock, "200 OK", "text/html; charset=utf-8", body.as_bytes()).await;
    }

    if is_get {
        let body = ORB_LAB_HTML.replace("__NOSD_TOKEN__", &token);
        return write_response(sock, "200 OK", "text/html; charset=utf-8", body.as_bytes()).await;
    }

    if !is_post_rpc {
        return write_response(
            sock,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"not found",
        )
        .await;
    }

    // Same-origin: browsers enviam Origin; clientes locais não.
    if let Some(origin) = header_value(&head, "origin") {
        if !is_local_origin(origin) {
            return write_response(
                sock,
                "403 Forbidden",
                "text/plain; charset=utf-8",
                b"origin denied",
            )
            .await;
        }
    }

    let ctype = header_value(&head, "content-type").unwrap_or("");
    if !ctype.to_ascii_lowercase().starts_with("application/json") {
        return write_response(
            sock,
            "415 Unsupported Media Type",
            "text/plain; charset=utf-8",
            b"application/json required",
        )
        .await;
    }

    let content_len = header_value(&head, "content-length")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if content_len > MAX_BODY {
        return write_response(
            sock,
            "413 Payload Too Large",
            "text/plain; charset=utf-8",
            b"body too large",
        )
        .await;
    }

    let header_end = head
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| head.find("\n\n").map(|i| i + 2))
        .unwrap_or(n);
    let mut body = buf[header_end.min(n)..n].to_vec();
    while body.len() < content_len {
        let m = tokio::time::timeout(READ_TIMEOUT, sock.read(&mut buf))
            .await
            .map_err(|_| anyhow::anyhow!("body read timeout"))??;
        if m == 0 {
            break;
        }
        body.extend_from_slice(&buf[..m]);
        if body.len() > MAX_BODY {
            return write_response(
                sock,
                "413 Payload Too Large",
                "text/plain; charset=utf-8",
                b"body too large",
            )
            .await;
        }
    }
    body.truncate(content_len);

    let req: Request = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            let msg = json!({ "ok": false, "error": format!("invalid json: {e}") });
            return write_response(
                sock,
                "400 Bad Request",
                "application/json",
                msg.to_string().as_bytes(),
            )
            .await;
        }
    };

    let token_ok = req
        .token
        .as_deref()
        .map(|t| nos_core::ct_eq(t.as_bytes(), token.as_bytes()))
        .unwrap_or(false);
    if !token_ok {
        let msg = json!({ "ok": false, "error": "invalid token" });
        return write_response(
            sock,
            "401 Unauthorized",
            "application/json",
            msg.to_string().as_bytes(),
        )
        .await;
    }

    let resp = super::dispatch(&state, &AuthContext::Tcp, req);
    let payload = serde_json::to_vec(&resp)?;
    write_response(sock, "200 OK", "application/json", &payload).await
}

async fn write_response<S>(
    sock: &mut S,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> anyhow::Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    sock.write_all(header.as_bytes()).await?;
    sock.write_all(body).await?;
    Ok(())
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{}:", name.to_ascii_lowercase());
    head.lines().find_map(|line| {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with(&needle) {
            Some(line[needle.len()..].trim())
        } else {
            None
        }
    })
}

fn is_local_origin(origin: &str) -> bool {
    origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
        || origin.starts_with("http://[::1]:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_value_is_case_insensitive() {
        let head = "POST /rpc HTTP/1.1\r\nContent-Type: application/json\r\nOrigin: http://127.0.0.1:7701\r\n\r\n";
        assert_eq!(header_value(head, "content-type"), Some("application/json"));
        assert_eq!(header_value(head, "origin"), Some("http://127.0.0.1:7701"));
        assert_eq!(header_value(head, "x-missing"), None);
    }

    #[test]
    fn local_origin_only() {
        assert!(is_local_origin("http://127.0.0.1:7701"));
        assert!(is_local_origin("http://localhost:7701"));
        assert!(is_local_origin("http://[::1]:7701"));
        assert!(!is_local_origin("http://evil.example"));
        assert!(!is_local_origin("https://127.0.0.1:7701"));
    }
}
