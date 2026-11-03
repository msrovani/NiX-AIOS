//! RPC prefs.get / prefs.set — UI kiosk prefs no data-dir.
use super::*;
use crate::prefs::{self, UiPrefs};
use nos_core::CapGate;
use nos_ipc::{Request, Response};
use serde_json::json;

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "prefs.get" => {
            let p = prefs::load(&state.data_dir);
            Response {
                id: req.id,
                ok: true,
                error: None,
                result: prefs::to_json(&p),
            }
        }
        "prefs.set" => {
            let patch = match req.params.as_object() {
                Some(o) => o,
                None => {
                    return Some(Response {
                        id: req.id,
                        ok: false,
                        error: Some("prefs.set: object params".into()),
                        result: json!(null),
                    });
                }
            };
            // Aceitar `{ ... }` directo ou `{ "patch": { ... } }`.
            let map = if let Some(inner) = patch.get("patch").and_then(|v| v.as_object()) {
                inner
            } else {
                patch
            };
            let mut p = prefs::load(&state.data_dir);
            if let Err(e) = prefs::merge_patch(&mut p, map) {
                return Some(Response {
                    id: req.id,
                    ok: false,
                    error: Some(e),
                    result: json!(null),
                });
            }
            match prefs::save(&state.data_dir, &p) {
                Ok(()) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: prefs::to_json(&p),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(e),
                    result: json!(null),
                },
            }
        }
        _ => return None,
    })
}

#[allow(dead_code)]
pub fn load_prefs(state: &AppState) -> UiPrefs {
    prefs::load(&state.data_dir)
}
