//! V8.1 (plano vampiro) — transporte **stdio** do MCP.
//!
//! Referência de transporte: `eco-tokens/templates/mcp-server.py` (linha a
//! linha, `id:null` em parse error). A fronteira continua a ser nossa: o
//! registry é server-authored e cada tool tem caps no `CapGate` — este módulo
//! só move bytes.
//!
//! Regras do stdio MCP:
//! - stdout é **só** JSON-RPC (nada de logs/`println!`);
//! - uma mensagem por linha; notificações (sem `id`) não têm resposta;
//! - JSON inválido → `-32700` com `id:null` e o processo continua.

#![forbid(unsafe_code)]

use super::{handle_jsonrpc, Registry, ToolExecutor};
use nos_core::CapGate;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// Serve o transporte stdio até EOF.
pub fn serve_stdio<R, W>(
    reg: &Registry,
    exec: &dyn ToolExecutor,
    gate: &CapGate,
    gen: u64,
    hitl_ok: bool,
    mut input: R,
    mut output: W,
) -> std::io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let out = match serde_json::from_str::<Value>(trimmed) {
            Ok(msg) => handle_jsonrpc(reg, exec, gate, gen, hitl_ok, &msg),
            Err(e) => Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": "Parse error", "data": e.to_string() },
            })),
        };
        if let Some(resp) = out {
            writeln!(output, "{resp}")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolSpec;
    use nos_core::CapBits;

    struct Echo;

    impl ToolExecutor for Echo {
        fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
            Ok(json!({ "tool": name, "args": args }))
        }
    }

    fn reg() -> Registry {
        let mut r = Registry::new();
        r.register(ToolSpec::new("nsd.health", "health", CapBits::NONE))
            .unwrap();
        r
    }

    fn run(input: &str) -> Vec<Value> {
        let gate = CapGate::new(CapBits::NONE, 1);
        let mut out = Vec::new();
        serve_stdio(
            &reg(),
            &Echo,
            &gate,
            1,
            false,
            std::io::Cursor::new(input.as_bytes().to_vec()),
            &mut out,
        )
        .expect("serve");
        String::from_utf8(out)
            .expect("utf8")
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("json line"))
            .collect()
    }

    #[test]
    fn initialize_then_list_then_call() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nsd.health","arguments":{}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#,
            "\n",
        );
        let out = run(input);
        // notificação não gera linha → 4 respostas
        assert_eq!(out.len(), 4);
        assert_eq!(out[0]["id"], 1);
        assert_eq!(out[0]["result"]["protocolVersion"], crate::PROTOCOL_VERSION);
        assert_eq!(out[1]["id"], 2);
        assert_eq!(out[1]["result"]["tools"].as_array().unwrap().len(), 1);
        assert_eq!(out[2]["id"], 3);
        assert!(out[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("nsd.health"));
        assert_eq!(out[3]["id"], 4);
        assert_eq!(out[3]["result"], json!({}));
    }

    #[test]
    fn parse_error_has_null_id_and_stream_continues() {
        let input = concat!(
            "não é json\n",
            r#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#,
            "\n",
            "\n",
        );
        let out = run(input);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["id"], Value::Null);
        assert_eq!(out[0]["error"]["code"], -32700);
        assert_eq!(out[1]["id"], 9);
    }

    #[test]
    fn unknown_method_is_32601() {
        let out = run("{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"resources/list\"}\n");
        assert_eq!(out[0]["error"]["code"], -32601);
    }
}
