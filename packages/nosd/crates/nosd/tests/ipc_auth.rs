//! S-01/S-02/S-03: identidade de transporte, token TCP e rotação.
//!
//! Sobe o binário `nosd` real e fala o framing IPC. Sem `agent`, sem token e
//! com `agent` legado devem ser negados; com token válido, health funciona.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
#[cfg(unix)]
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Ports ef?meros n?o s?o reservados; serializa os testes que sobem nosd.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

struct Daemon {
    child: Child,
    port: u16,
    token: String,
    #[cfg(unix)]
    socket: Option<PathBuf>,
    _dir: TempDir,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start() -> Daemon {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let token = "test-token-0123456789abcdef".to_string();
    let args = vec![
        "--foreground".to_string(),
        "--tcp".to_string(),
        format!("127.0.0.1:{port}"),
        "--data-dir".to_string(),
        dir.path().to_string_lossy().to_string(),
        "--orb-lab".to_string(),
        String::new(),
        "--hermes-tick-secs".to_string(),
        "0".to_string(),
        "--ipc-token".to_string(),
        token.clone(),
    ];
    let child = Command::new(env!("CARGO_BIN_EXE_nosd"))
        .args(&args)
        .env_remove("NOSD_NIX_RING")
        .env_remove("NOSD_LANDLOCK")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn nosd");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut child = child;
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("nosd exited before listening: {status}");
        }
        if Instant::now() > deadline {
            panic!("nosd did not listen on 127.0.0.1:{port}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    Daemon {
        child,
        port,
        token,
        #[cfg(unix)]
        socket: None,
        _dir: dir,
    }
}

fn rpc_with(
    daemon: &Daemon,
    token: Option<&str>,
    agent: Option<&str>,
    method: &str,
    params: Value,
) -> Value {
    let mut req = json!({
        "id": 1,
        "method": method,
        "cap_gen": 1,
        "params": params,
    });
    if let Some(token) = token {
        req["token"] = json!(token);
    }
    if let Some(agent) = agent {
        req["agent"] = json!(agent);
    }
    let body = serde_json::to_vec(&req).unwrap();
    let mut sock = TcpStream::connect(("127.0.0.1", daemon.port)).unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.write_all(&(body.len() as u32).to_le_bytes()).unwrap();
    sock.write_all(&body).unwrap();
    sock.shutdown(Shutdown::Write).unwrap();

    let mut len = [0u8; 4];
    sock.read_exact(&mut len).unwrap();
    let n = u32::from_le_bytes(len) as usize;
    let mut buf = vec![0u8; n];
    sock.read_exact(&mut buf).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn rpc(daemon: &Daemon, token: Option<&str>, method: &str, params: Value) -> Value {
    rpc_with(daemon, token, None, method, params)
}

#[test]
fn tcp_without_token_is_denied() {
    let _serial = serial();
    let daemon = start();
    let r = rpc(&daemon, None, "health", json!({}));
    assert_eq!(r["ok"], false, "{r}");
    let err = r["error"].as_str().unwrap_or("");
    assert!(err.contains("unauthenticated"), "{r}");
    assert_eq!(r["result"]["profile"], "Unknown");
}

#[test]
fn legacy_agent_does_not_authenticate() {
    let _serial = serial();
    let daemon = start();
    for agent in ["hermes", "nosctl", "jarbas", ""] {
        let r = rpc_with(&daemon, None, Some(agent), "health", json!({}));
        assert_eq!(r["ok"], false, "agent={agent:?} should not elevate: {r}");
    }
}

#[test]
fn valid_token_health_works() {
    let _serial = serial();
    let daemon = start();
    let r = rpc(&daemon, Some(&daemon.token), "health", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["result"]["status"], "ok");
}

#[test]
fn hitl_grant_is_single_use_and_kind_bound() {
    let _serial = serial();
    let daemon = start();
    let token = daemon.token.clone();

    let opened = rpc(
        &daemon,
        Some(&token),
        "ui.open",
        json!({
            "kind": "hitl_confirm",
            "title": "self heal",
            "grant_kind": "self.heal",
        }),
    );
    assert_eq!(opened["ok"], true, "{opened}");
    let wid = opened["result"]["id"].as_u64().expect("window id");

    let acted = rpc(
        &daemon,
        Some(&token),
        "ui.action",
        json!({ "window_id": wid, "action": "submit", "values": {} }),
    );
    assert_eq!(acted["ok"], true, "{acted}");
    let confirm = acted["result"]["confirm_id"]
        .as_str()
        .expect("confirm_id")
        .to_string();

    let heal = rpc(
        &daemon,
        Some(&token),
        "self.heal.evaluate",
        json!({
            "id": "x",
            "action": "nix_fragment",
            "target": "/etc/nixos/ai-managed.nix",
            "confirm_id": confirm,
        }),
    );
    assert_eq!(heal["ok"], true, "{heal}");

    let replay = rpc(
        &daemon,
        Some(&token),
        "self.heal.evaluate",
        json!({
            "id": "x",
            "action": "nix_fragment",
            "target": "/etc/nixos/ai-managed.nix",
            "confirm_id": confirm,
        }),
    );
    assert_eq!(replay["ok"], false, "grant must be single-use: {replay}");

    let without = rpc(
        &daemon,
        Some(&token),
        "self.heal.evaluate",
        json!({ "id": "x", "action": "nix_fragment", "target": "/etc/nixos/ai-managed.nix" }),
    );
    assert_eq!(without["ok"], false, "no grant must be denied: {without}");
}

#[test]
fn blob_fetch_requires_single_use_grant() {
    let daemon = start();
    let token = daemon.token.clone();

    let opened = rpc(
        &daemon,
        Some(&token),
        "ui.open",
        json!({
            "kind": "hitl_confirm",
            "title": "blob fetch",
            "grant_kind": "blob.fetch",
        }),
    );
    assert_eq!(opened["ok"], true, "{opened}");
    let wid = opened["result"]["id"].as_u64().expect("window id");

    let acted = rpc(
        &daemon,
        Some(&token),
        "ui.action",
        json!({ "window_id": wid, "action": "submit", "values": {} }),
    );
    let confirm = acted["result"]["confirm_id"]
        .as_str()
        .expect("confirm_id")
        .to_string();

    // Grant válido passa o HITL e tenta a rede (falha em :1, não em HITL).
    let fetch = rpc(
        &daemon,
        Some(&token),
        "model.fetch",
        json!({
            "url": "http://127.0.0.1:1/tiny.gguf",
            "filename": "tiny.gguf",
            "confirm_id": confirm,
        }),
    );
    assert_eq!(fetch["ok"], false, "{fetch}");
    let err = fetch["error"].as_str().unwrap_or("");
    assert!(!err.contains("await JARBAS HITL"), "{fetch}");

    // Replay do mesmo grant deve voltar a pedir HITL.
    let replay = rpc(
        &daemon,
        Some(&token),
        "model.fetch",
        json!({
            "url": "http://127.0.0.1:1/tiny.gguf",
            "filename": "tiny.gguf",
            "confirm_id": confirm,
        }),
    );
    assert_eq!(replay["ok"], false, "{replay}");
    let err = replay["error"].as_str().unwrap_or("");
    assert!(err.contains("await JARBAS HITL"), "{replay}");
}

#[test]
fn wifi_requires_hitl_grant_without_admin_legacy() {
    let _serial = serial();
    let daemon = start();
    let r = rpc(
        &daemon,
        Some(&daemon.token),
        "wifi.connect",
        json!({ "ssid": "lab", "password": "x" }),
    );
    assert_eq!(r["ok"], false, "{r}");
    let err = r["error"].as_str().unwrap_or("");
    assert!(err.contains("HITL") || err.contains("SITE_POLICY"), "{r}");
}

#[test]
fn nix_ring_is_disarmed_by_default() {
    let _serial = serial();
    let daemon = start();
    let r = rpc(
        &daemon,
        Some(&daemon.token),
        "nix.apply",
        json!({ "hitl_confirmed": true }),
    );
    assert_eq!(r["ok"], false, "nix.apply must be disarmed: {r}");
    let err = r["error"].as_str().unwrap_or("");
    assert!(err.contains("NIX_RING_DISABLED"), "{r}");
}

#[cfg(unix)]
fn start_unix() -> Daemon {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nosd.sock");
    let token = "test-unix-token-0123456789".to_string();
    let args = vec![
        "--foreground".to_string(),
        "--socket".to_string(),
        socket.to_string_lossy().to_string(),
        "--data-dir".to_string(),
        dir.path().to_string_lossy().to_string(),
        "--orb-lab".to_string(),
        String::new(),
        "--hermes-tick-secs".to_string(),
        "0".to_string(),
        "--ipc-token".to_string(),
        token.clone(),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_nosd"))
        .args(&args)
        .env_remove("NOSD_NIX_RING")
        .env_remove("NOSD_LANDLOCK")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn nosd unix");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("nosd unix exited before listening: {status}");
        }
        if Instant::now() > deadline {
            panic!("nosd unix did not listen on {}", socket.display());
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    Daemon {
        child,
        port: 0,
        token,
        socket: Some(socket),
        _dir: dir,
    }
}

#[cfg(unix)]
fn unix_rpc(
    daemon: &Daemon,
    token: Option<&str>,
    agent: Option<&str>,
    method: &str,
    params: Value,
) -> Value {
    let mut req = json!({
        "id": 1,
        "method": method,
        "cap_gen": 1,
        "params": params,
    });
    if let Some(token) = token {
        req["token"] = json!(token);
    }
    if let Some(agent) = agent {
        req["agent"] = json!(agent);
    }
    let body = serde_json::to_vec(&req).unwrap();
    let path = daemon.socket.as_ref().unwrap();
    let mut sock = std::os::unix::net::UnixStream::connect(path).unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.write_all(&(body.len() as u32).to_le_bytes()).unwrap();
    sock.write_all(&body).unwrap();
    sock.shutdown(Shutdown::Write).unwrap();

    let mut len = [0u8; 4];
    sock.read_exact(&mut len).unwrap();
    let n = u32::from_le_bytes(len) as usize;
    let mut buf = vec![0u8; n];
    sock.read_exact(&mut buf).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

#[cfg(unix)]
#[test]
fn unix_token_grants_admin_and_peer_creds_do_not() {
    let _serial = serial();
    let daemon = start_unix();
    let uid = unsafe { libc::getuid() };

    // Sem token: peer creds do mesmo uid do daemon => Jarbas (sem NIX_WRITE).
    // Root é LocalAdmin; nesse caso o teste não consegue provar a negação.
    let without = unix_rpc(
        &daemon,
        None,
        Some("hermes"),
        "nix.propose",
        json!({ "nix": "{ ... }: { }" }),
    );
    if uid != 0 {
        assert_eq!(
            without["ok"], false,
            "peer creds must not elevate: {without}"
        );
    }

    // Com token: LocalAdmin, mesmo no socket Unix. `security.rotate` exige
    // NIX_WRITE|HITL e não depende de nix-instantiate (portátil no CI).
    let with_token = unix_rpc(
        &daemon,
        Some(&daemon.token),
        None,
        "security.rotate",
        json!({}),
    );
    assert_eq!(
        with_token["ok"], true,
        "unix token must grant admin: {with_token}"
    );
}

#[test]
fn rotate_revokes_old_token() {
    let _serial = serial();
    let daemon = start();
    let rotated = rpc(&daemon, Some(&daemon.token), "security.rotate", json!({}));
    assert_eq!(rotated["ok"], true, "{rotated}");
    let new_token = rotated["result"]["token"].as_str().unwrap().to_string();
    assert_ne!(new_token, daemon.token);

    let old = rpc(&daemon, Some(&daemon.token), "health", json!({}));
    assert_eq!(old["ok"], false, "old token must be revoked: {old}");

    let fresh = rpc(&daemon, Some(&new_token), "health", json!({}));
    assert_eq!(fresh["ok"], true, "new token must work: {fresh}");
}

// ---- S-08/S-09: regressoes de endurecimento ------------------------------

#[cfg(unix)]
#[test]
fn unix_socket_is_0660() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = serial();
    let daemon = start_unix();
    let sock = daemon.socket.as_ref().expect("socket unix");
    let mode = std::fs::metadata(sock)
        .expect("stat socket")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o660, "S-09: socket deve ser 0660 (foi {mode:o})");
}

#[test]
fn unknown_method_is_denied_not_ignored() {
    let _serial = serial();
    let daemon = start();
    // Fail-closed: um metodo inexistente nao pode devolver ok=true.
    let r = rpc(&daemon, Some(&daemon.token), "nao.existe", json!({}));
    assert_eq!(r["ok"], false, "unknown method must fail closed: {r}");
}

#[test]
fn admin_method_requires_token() {
    let _serial = serial();
    let daemon = start();
    // `security.rotate` e admin: sem token TCP tem de ser negado antes do efeito.
    let r = rpc(&daemon, None, "security.rotate", json!({}));
    assert_eq!(r["ok"], false, "admin sem token: {r}");
    // E o token antigo continua a funcionar (nada rodou).
    let h = rpc(&daemon, Some(&daemon.token), "health", json!({}));
    assert_eq!(h["ok"], true, "{h}");
}
