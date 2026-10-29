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
const MAX_BODY: usize = 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// V9.4 — superfície do bridge do kiosk (browser). **Só leitura + chat/HITL.**
///
/// Esta lista é a fronteira: o perfil `Jarbas` tem FS/NET/INFER/HITL, por isso
/// é aqui (e não nas caps) que se impede o browser de escrever. Métodos novos
/// exigem teste de contrato (`kiosk_methods_are_scope_limited`).
const KIOSK_METHODS: &[&str] = &[
    "health",
    "score",
    "cognitive.tick",
    "orb.get",
    "orb.set",
    "orb.sense",
    "ui.list",
    "ui.action",
    "ui.pending",
    "chat",
    "voice.turn",
    "observe.energy",
];

/// Lab cinemático: GET `/` → HTML (cam+mic+padrões); GET `/transmute` → wizard;
/// POST `/rpc` → dispatch autenticado por token.
pub async fn serve_orb_lab(addr: String, state: Arc<AppState>) -> anyhow::Result<()> {
    // Bancada sem launcher: garante chave bootstrap para #bootstrap=<valor>.
    state.ensure_jarbas_ui_bootstrap();
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
    let is_get = first.starts_with("GET / ")
        || first.starts_with("GET /index")
        || first.starts_with("GET /?");

    if is_get {
        // Never place the process IPC token in browser-visible HTML/JavaScript.
        let body = ORB_LAB_HTML.replace("__JARBAS_SESSION__", "");
        return write_response(sock, "200 OK", "text/html; charset=utf-8", body.as_bytes()).await;
    }

    if first.starts_with("POST /ui/session") {
        let origin = header_value(&head, "origin").unwrap_or("");
        if !is_local_origin(origin) {
            return write_response(
                sock,
                "403 Forbidden",
                "text/plain; charset=utf-8",
                b"origin denied",
            )
            .await;
        }
        let fragment = first
            .split_whitespace()
            .nth(1)
            .and_then(|target| target.split_once('?'))
            .and_then(|(_, query)| query.strip_prefix("key="))
            .unwrap_or("");
        if !state.validate_jarbas_ui_bootstrap(fragment) {
            return write_response(
                sock,
                "401 Unauthorized",
                "text/plain; charset=utf-8",
                b"invalid or expired UI bootstrap key",
            )
            .await;
        }
        // Cookie de sessao (sem Max-Age): vive enquanto o kiosk; o TTL no
        // servidor e deslizante e renovado pelo poll do HUD.
        let cookie = format!(
            "Set-Cookie: jarbas_ui={}; HttpOnly; SameSite=Strict; Path=/\r\n",
            state.issue_jarbas_ui_session()
        );
        return write_response_with_headers(
            sock,
            "204 No Content",
            "text/plain; charset=utf-8",
            &[],
            &[("Set-Cookie", cookie.as_str())],
        )
        .await;
    }

    if first.starts_with("POST /ui/logout") {
        let origin = header_value(&head, "origin").unwrap_or("");
        let cookie = header_value(&head, "cookie").unwrap_or("");
        if !is_local_origin(origin) || !state.valid_jarbas_ui_session(cookie) {
            return write_response(
                sock,
                "401 Unauthorized",
                "text/plain; charset=utf-8",
                b"UI session required",
            )
            .await;
        }
        state.revoke_jarbas_ui_session(cookie);
        return write_response_with_headers(
            sock,
            "204 No Content",
            "text/plain; charset=utf-8",
            &[],
            &[(
                "Set-Cookie",
                "Set-Cookie: jarbas_ui=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0\r\n",
            )],
        )
        .await;
    }

    if first.starts_with("GET /transmute") {
        return write_response(
            sock,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            b"Transmute is not available from the JARBAS kiosk",
        )
        .await;
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

    // Browser UI: require a same-origin, short-lived HttpOnly session cookie.
    let origin = header_value(&head, "origin").unwrap_or("");
    if !is_local_origin(origin) {
        return write_response(
            sock,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            b"origin denied",
        )
        .await;
    }
    let cookie = header_value(&head, "cookie").unwrap_or("");
    if !state.valid_jarbas_ui_session(cookie) {
        return write_response(
            sock,
            "401 Unauthorized",
            "text/plain; charset=utf-8",
            b"UI session required",
        )
        .await;
    }
    if header_value(&head, "x-jarbas-ui-request").unwrap_or("") != "1" {
        return write_response(
            sock,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            b"UI request marker required",
        )
        .await;
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

    let allowed_methods = KIOSK_METHODS;
    if !allowed_methods.contains(&req.method.as_str()) {
        return write_response(
            sock,
            "403 Forbidden",
            "application/json",
            br#"{"ok":false,"error":"method not available to JARBAS kiosk"}"#,
        )
        .await;
    }

    if req.token.is_some() {
        let msg = json!({ "ok": false, "error": "browser token fields are forbidden" });
        return write_response(
            sock,
            "400 Bad Request",
            "application/json",
            msg.to_string().as_bytes(),
        )
        .await;
    }

    let resp = super::dispatch(&state, &AuthContext::JarbasUi, req);
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
    write_response_with_headers(sock, status, content_type, body, &[]).await
}

async fn write_response_with_headers<S>(
    sock: &mut S,
    status: &str,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&str, &str)],
) -> anyhow::Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    let mut header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (_, value) in extra_headers {
        header.push_str(value);
    }
    header.push_str("\r\n");
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
    // Origem exacta (nunca por prefixo: "127.0.0.1.evil.example" seria
    // aceite). O launcher do kiosk usa 127.0.0.1; localhost e [::1] ficam
    // para bancada manual na mesma porta. A autorizacao real e a cookie
    // HttpOnly + chave bootstrap, nao esta verificacao defensiva.
    matches!(
        origin,
        "http://127.0.0.1:7701" | "http://localhost:7701" | "http://[::1]:7701"
    )
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
        // Origem exacta do launcher do kiosk; prefixos sao rejeitados
        // (127.0.0.1.example, https, outras portas).
        assert!(is_local_origin("http://127.0.0.1:7701"));
        assert!(is_local_origin("http://localhost:7701"));
        assert!(is_local_origin("http://[::1]:7701"));
        assert!(!is_local_origin(""));
        assert!(!is_local_origin("http://127.0.0.1:7702"));
        assert!(!is_local_origin("http://127.0.0.1.evil.example"));
        assert!(!is_local_origin("http://evil.example"));
        assert!(!is_local_origin("https://127.0.0.1:7701"));
    }

    /// V9.4: contrato da allowlist do kiosk.
    #[test]
    fn kiosk_methods_are_scope_limited() {
        use nos_core::CapBits;
        let mut seen = std::collections::BTreeSet::new();
        for m in KIOSK_METHODS {
            assert!(seen.insert(*m), "método duplicado na allowlist: {m}");
            let req = nos_ipc::Request {
                id: 1,
                method: (*m).to_string(),
                caps: 0,
                cap_gen: 0,
                agent: "test".into(),
                token: None,
                params: serde_json::json!({}),
            };
            let need = req.required_cap();
            // Só INFER (chat/voz), HITL (ui.action) ou NONE. Nunca FS/NET/
            // NIX_WRITE/AGENT_SPAWN/MEMORY — o perfil Jarbas tem-nos.
            assert!(
                need == CapBits::NONE || need == CapBits::INFER || need == CapBits::HITL,
                "{m} exige {need:?}: fora do âmbito do kiosk"
            );
        }
        for m in ["observe.energy", "ui.pending"] {
            assert!(KIOSK_METHODS.contains(&m), "{m} devia estar exposto");
        }
        for forbidden in [
            "nix.apply",
            "nix.confirm",
            "nix.rollback",
            "skill.install",
            "skill.import",
            "skill.push",
            "model.fetch",
            "model.load",
            "transmute.run",
            "wifi.connect",
            "security.rotate",
            "systemctl.scoped",
            "self.heal.evaluate",
            "mcp.call",
            "crdt.put",
            "crdt.sync",
            "memory.remember",
        ] {
            assert!(
                !KIOSK_METHODS.contains(&forbidden),
                "{forbidden} NÃO pode estar na allowlist do kiosk"
            );
        }
    }
}
