//! M9 — skills wasmi: módulo content-addressed + registo no CRDT + execução
//! frugal (fuel 100k, heap 256 KiB, **sem capacidades** por omissão).
//!
//! `skill.install` aceita `wat` (texto, compilado no servidor) ou `wasm_b64`.
//! O hash do módulo é FNV-1a64 (id de conteúdo; a integridade forte virá com
//! BLAKE3 no M8.1). A skill activa é `skill/<nome>/active` no CRDT — trocar de
//! versão é uma op assinada, rollback é apontar para o hash anterior.

use super::*;
use nos_ipc::{Request, Response};
use serde_json::json;

fn skills_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("skills")
}

fn content_id(bytes: &[u8]) -> String {
    format!("{:016x}", nos_mesh::mesh_knowledge::fnv1a64(bytes))
}

fn err(id: u64, msg: impl Into<String>) -> Response {
    Response {
        id,
        ok: false,
        error: Some(msg.into()),
        result: json!(null),
    }
}

pub(crate) fn dispatch(state: &AppState, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "skill.install" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if name.is_empty() || name.contains('/') || name.len() > 64 {
                return Some(err(req.id, "skill.install: nome invalido"));
            }
            let bytes: Vec<u8> = if let Some(wat) = req.params.get("wat").and_then(|v| v.as_str()) {
                match wat::parse_str(wat) {
                    Ok(b) => b,
                    Err(e) => return Some(err(req.id, format!("wat: {e}"))),
                }
            } else if let Some(b64) = req.params.get("wasm_b64").and_then(|v| v.as_str()) {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                    Ok(b) => b,
                    Err(e) => return Some(err(req.id, format!("base64: {e}"))),
                }
            } else {
                return Some(err(req.id, "skill.install requer wat ou wasm_b64"));
            };
            if bytes.len() > 4 * 1024 * 1024 {
                return Some(err(req.id, "modulo acima de 4 MiB"));
            }
            // Valida antes de gravar (fail-closed).
            if let Err(e) =
                nos_wasm::FrugalRuntime::new().run_i32(&bytes, nos_core::CapBits::NONE, 1, 0)
            {
                if !matches!(e, nos_wasm::WasmError::Fuel) {
                    return Some(err(req.id, format!("modulo invalido: {e}")));
                }
            }
            let dir = skills_dir(state);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return Some(err(req.id, format!("skills dir: {e}")));
            }
            let cid = content_id(&bytes);
            let path = dir.join(format!("{cid}.wasm"));
            if let Err(e) = std::fs::write(&path, &bytes) {
                return Some(err(req.id, format!("write: {e}")));
            }
            state.mesh.crdt_put(
                state.mesh.local_id(),
                &format!("skill/{name}/active"),
                &cid,
                0,
            );
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!({
                    "name": name,
                    "hash": cid,
                    "bytes": bytes.len(),
                    "path": path.display().to_string(),
                }),
            }
        }
        "skill.list" => {
            let store = state.mesh.crdt_snapshot();
            let items: Vec<serde_json::Value> = store
                .entries
                .iter()
                .filter_map(|(k, e)| {
                    k.strip_prefix("skill/")
                        .and_then(|r| r.strip_suffix("/active"))
                        .map(|name| json!({ "name": name, "hash": e.value }))
                })
                .collect();
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: json!(items),
            }
        }
        "skill.run" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arg = req.params.get("arg").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let key = format!("skill/{name}/active");
            let Some(entry) = state.mesh.crdt_get(&key) else {
                return Some(err(req.id, format!("skill '{name}' nao instalada")));
            };
            let path = skills_dir(state).join(format!("{}.wasm", entry.value));
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return Some(err(req.id, format!("modulo {}: {e}", entry.value))),
            };
            // Sem capacidades por omissão: `aios_ai::infer` devolve -1.
            let rt = nos_wasm::FrugalRuntime::new();
            match rt.run_i32(&bytes, nos_core::CapBits::NONE, 1, arg) {
                Ok(v) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: json!({ "name": name, "arg": arg, "value": v, "hash": entry.value }),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e.to_string()),
                    result: json!({ "name": name, "hash": entry.value }),
                },
            }
        }
        _ => return None,
    })
}
