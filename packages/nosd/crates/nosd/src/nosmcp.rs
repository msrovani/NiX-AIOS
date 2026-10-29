//! V8.1 (plano vampiro) — `nosmcp`: bridge MCP **stdio read-only** para o nosd.
//!
//! Um agente de código (Cursor/eco-tokens, Claude Code, …) fala MCP por stdio;
//! este binário traduz para IPC e **só** expõe leitura. Não há tool de escrita,
//! não abre a allowlist do kiosk e não usa `ipc.token` para elevar o peer: o
//! perfil é o do transporte (Unix `SO_PEERCRED` ou token se fornecido).
//!
//! stdout é **exclusivamente** JSON-RPC (diagnóstico vai para stderr).

use clap::Parser;
use nos_core::{CapBits, CapGate};
use nos_ipc::{decode_body, encode, take_frame, Request, Response};
use nos_mcp::{Registry, ToolExecutor, ToolSpec};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/run/nosd/nosd.sock")]
    socket: String,
    #[arg(long)]
    tcp: Option<String>,
    #[arg(long, env = "NOSD_IPC_TOKEN", hide_env_values = true)]
    token: Option<String>,
    #[arg(long, env = "NOSD_IPC_TOKEN_FILE")]
    token_file: Option<PathBuf>,
}

trait Rw: Read + Write {}
impl<T: Read + Write> Rw for T {}

struct IpcExecutor {
    socket: String,
    tcp: Option<String>,
    token: Option<String>,
}

impl IpcExecutor {
    fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        let req = Request {
            id: 1,
            method: method.to_string(),
            caps: 0, // ignored by daemon (server-authored)
            cap_gen: 1,
            agent: "nosmcp".to_string(),
            token: self.token.clone(),
            params,
        };
        let enc = encode(&req).map_err(|e| e.to_string())?;

        let mut stream: Box<dyn Rw> = if let Some(addr) = &self.tcp {
            Box::new(std::net::TcpStream::connect(addr).map_err(|e| format!("tcp {addr}: {e}"))?)
        } else {
            #[cfg(unix)]
            {
                Box::new(
                    std::os::unix::net::UnixStream::connect(&self.socket)
                        .map_err(|e| format!("socket {}: {e}", self.socket))?,
                )
            }
            #[cfg(not(unix))]
            {
                // No Windows não há Unix socket: `--socket` só é usado se já
                // for `host:port`; senão cai para o TCP de bancada.
                let addr = if self.socket.contains(':') && !self.socket.starts_with('/') {
                    self.socket.clone()
                } else {
                    "127.0.0.1:7700".to_string()
                };
                Box::new(
                    std::net::TcpStream::connect(&addr)
                        .map_err(|e| format!("tcp fallback {addr}: {e}"))?,
                )
            }
        };

        stream.write_all(&enc).map_err(|e| e.to_string())?;
        let mut acc = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("daemon fechou a ligação (eof)".into());
            }
            acc.extend_from_slice(&buf[..n]);
            if let Some((body, _)) = take_frame(&acc).map_err(|e| e.to_string())? {
                let resp: Response = decode_body(&body).map_err(|e| e.to_string())?;
                if !resp.ok {
                    return Err(resp.error.unwrap_or_else(|| "erro sem detalhe".into()));
                }
                return Ok(resp.result);
            }
        }
    }
}

impl ToolExecutor for IpcExecutor {
    fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
        let (method, params) = match name {
            "nsd.health" => ("health", json!({})),
            "nsd.score" => ("score", json!({})),
            "nsd.energy" => ("observe.energy", json!({})),
            "nsd.pending" => ("ui.pending", json!({})),
            "nsd.trace" => (
                "cognitive.trace",
                json!({ "last": args.get("last").and_then(|v| v.as_u64()).unwrap_or(8) }),
            ),
            other => return Err(format!("unknown tool: {other}")),
        };
        self.rpc(method, params)
    }
}

/// Registry **read-only** e server-authored (não vem do cliente).
fn registry() -> Registry {
    let mut reg = Registry::new();
    for (name, desc) in [
        ("nsd.health", "estado do daemon (leitura)"),
        ("nsd.score", "score honesto do runtime (leitura)"),
        (
            "nsd.energy",
            "energia por janela + custo local (leitura; source/scope explícitos)",
        ),
        ("nsd.pending", "fila HITL: tier + TTL, sem nonce (leitura)"),
        (
            "nsd.trace",
            "últimos actos OPAR: domínio + comprimentos, sem notas (leitura)",
        ),
    ] {
        let _ = reg.register(ToolSpec::new(name, desc, CapBits::NONE));
    }
    reg
}

fn load_token(explicit: Option<String>, file: Option<PathBuf>) -> Option<String> {
    if let Some(t) = explicit {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(p) = file {
        paths.push(p);
    }
    if let Ok(p) = std::env::var("NOSD_IPC_TOKEN_FILE") {
        paths.push(PathBuf::from(p));
    }
    paths.push(PathBuf::from("/run/nosd/ipc.token"));
    paths.push(PathBuf::from("/var/lib/nosd/ipc.token"));
    paths.push(PathBuf::from("data/ipc.token"));
    for p in paths {
        if let Ok(t) = std::fs::read_to_string(&p) {
            let t = t.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();
    let token = load_token(args.token.clone(), args.token_file.clone());
    if args.tcp.is_some() && token.is_none() {
        eprintln!("nosmcp: TCP requires --token/--token-file (or NOSD_IPC_TOKEN*)");
        return std::process::ExitCode::from(2);
    }
    let exec = IpcExecutor {
        socket: args.socket,
        tcp: args.tcp,
        token,
    };
    // Piso NONE: as tools do registry também exigem NONE. Nenhuma escrita.
    let gate = CapGate::new(CapBits::NONE, 1);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    match nos_mcp::stdio::serve_stdio(
        &registry(),
        &exec,
        &gate,
        1,
        false,
        stdin.lock(),
        stdout.lock(),
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nosmcp: {e}");
            std::process::ExitCode::from(1)
        }
    }
}
