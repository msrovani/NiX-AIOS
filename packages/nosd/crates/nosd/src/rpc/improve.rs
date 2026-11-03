//! CHJ improve.* — check (opt-in probes allowlist) + propose (card HITL).
use super::*;
use nos_core::{reported_rate, CapGate};
use nos_hermes::{
    fingerprint_of, improve_propose_open, ImprovePropose, DEFAULT_FP_TTL_MS,
};
use nos_ipc::{Request, Response};
use serde_json::json;
use std::path::Path;

fn improve_check_result(state: &AppState) -> serde_json::Value {
    let enabled = std::env::var("NOSD_IMPROVE_CHECK").ok().as_deref() == Some("1");
    if !enabled {
        let nix_u = reported_rate(0, 0, &["nixos_version"], "nixos-version", &[]).unwrap();
        let kern_u =
            reported_rate(0, 0, &["kernel_release"], "kernel.running", &[]).unwrap();
        return json!({
            "enabled": false,
            "nix": nix_u,
            "kernel": kern_u,
            "note": "opt-in: NOSD_IMPROVE_CHECK=1; probes só ficheiros/env locais (sem phone-home)",
        });
    }
    let mut kernel_measured = 0u64;
    let kernel_running = probe_kernel_running(&mut kernel_measured);
    let kernel_candidate = probe_kernel_candidate(&state.data_dir);
    let mut nix_measured = 0u64;
    let nix_version = probe_nixos_version(&mut nix_measured);
    let flake_rev = probe_flake_rev();
    let k_rate = reported_rate(
        kernel_measured,
        1,
        &["kernel_release"],
        "kernel.running",
        &[],
    )
    .unwrap();
    let n_rate = reported_rate(
        nix_measured,
        1,
        &["nixos_version"],
        "nixos-version",
        &[],
    )
    .unwrap();

    let mut suggestions = Vec::new();
    if let (Some(run), Some(cand)) = (&kernel_running, &kernel_candidate) {
        if run != &cand.version {
            suggestions.push(json!({
                "kind": "kernel",
                "importance": cand.importance,
                "current": run,
                "candidate": cand.version,
                "accept_effect": "abre nix.propose (ring desarmado — sem switch automático)",
            }));
        }
    }

    json!({
        "enabled": true,
        "kernel": {
            "running": kernel_running,
            "candidate": kernel_candidate.as_ref().map(|c| &c.version),
            "candidate_importance": kernel_candidate.as_ref().map(|c| &c.importance),
            "rate": k_rate,
        },
        "nix": {
            "nixos_version": nix_version,
            "flake_rev": flake_rev,
            "rate": n_rate,
        },
        "suggestions": suggestions,
        "honesty": "só valores observados em uname/nixos-version/ficheiros pinados; sem scrape",
    })
}

struct KernelCandidate {
    version: String,
    importance: String,
}

fn probe_kernel_candidate(data_dir: &Path) -> Option<KernelCandidate> {
    if let Ok(v) = std::env::var("NOSD_KERNEL_CANDIDATE") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            let importance = std::env::var("NOSD_KERNEL_IMPORTANCE")
                .unwrap_or_else(|_| "medium".into());
            return Some(KernelCandidate {
                version: v,
                importance: importance.trim().to_string(),
            });
        }
    }
    let path = data_dir.join("improve").join("kernel_candidate.txt");
    let raw = std::fs::read_to_string(path).ok()?;
    let mut lines = raw.lines().map(str::trim).filter(|l| !l.is_empty());
    let version = lines.next()?.to_string();
    let importance = lines
        .next()
        .unwrap_or("medium")
        .trim()
        .to_string();
    Some(KernelCandidate {
        version,
        importance,
    })
}

fn probe_flake_rev() -> serde_json::Value {
    let path = match std::env::var("NOSD_FLAKE_LOCK") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => {
            return json!({
                "rev": null,
                "basis": "unmeasured",
                "note": "defina NOSD_FLAKE_LOCK=caminho/flake.lock",
            });
        }
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return json!({ "rev": null, "basis": "unmeasured", "note": "flake.lock unreadable" });
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return json!({ "rev": null, "basis": "unmeasured", "note": "flake.lock JSON invalid" });
    };
    let rev = v
        .pointer("/nodes/nixpkgs/locked/rev")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    if let Some(r) = rev {
        json!({ "rev": r, "basis": "observed", "path": path })
    } else {
        json!({ "rev": null, "basis": "unmeasured", "note": "nodes.nixpkgs.locked.rev absent" })
    }
}

#[cfg(unix)]
fn probe_kernel_running(measured: &mut u64) -> Option<String> {
    use std::process::Command;
    let out = Command::new("/usr/bin/uname")
        .args(["-r"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return None;
    }
    *measured = 1;
    Some(s)
}

#[cfg(not(unix))]
fn probe_kernel_running(_measured: &mut u64) -> Option<String> {
    None
}

fn probe_nixos_version(measured: &mut u64) -> Option<String> {
    use std::process::Command;
    let candidates = [
        "/run/current-system/sw/bin/nixos-version",
        "/nix/var/nix/profiles/system/sw/bin/nixos-version",
    ];
    for path in candidates {
        if !std::path::Path::new(path).is_file() {
            continue;
        }
        let out = Command::new(path).output().ok().filter(|o| o.status.success())?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            *measured = 1;
            return Some(s);
        }
    }
    None
}

pub(crate) fn dispatch(state: &AppState, _gate: &CapGate, req: Request) -> Option<Response> {
    Some(match req.method.as_str() {
        "improve.check" => Response {
            id: req.id,
            ok: true,
            error: None,
            result: improve_check_result(state),
        },
        "improve.propose" => {
            let mut p: ImprovePropose = match serde_json::from_value(req.params.clone()) {
                Ok(v) => v,
                Err(e) => {
                    // Atalho DIAG: { "kind": "system.suspend" } mínimo
                    if req.params.get("kind").and_then(|v| v.as_str()) == Some("system.suspend")
                    {
                        ImprovePropose {
                            kind: "system.suspend".into(),
                            title: "Suspender o sistema".into(),
                            reason: "Proposta manual (DIAG) — idle/DPMS já passou; suspend \
                                     exige autorização."
                                .into(),
                            importance: "medium".into(),
                            current: "ligado".into(),
                            candidate: "systemctl suspend".into(),
                            evidence: vec!["ui:diag_propose_suspend".into()],
                            alternatives: vec!["ficar acordado".into(), "só blank ecrã".into()],
                            grant_kind: "system.suspend".into(),
                            accept_effect: "Com grant: systemctl suspend (metal AWAITING). \
                                            Nunca automático."
                                .into(),
                            fingerprint: String::new(),
                        }
                    } else {
                        return Some(Response {
                            id: req.id,
                            ok: false,
                            error: Some(format!("improve.propose params: {e}")),
                            result: json!(null),
                        });
                    }
                }
            };
            if p.fingerprint.is_empty() {
                p.fingerprint = fingerprint_of(&p);
            }
            // Hibernate: só se capability ok
            if p.kind == "system.hibernate" || p.grant_kind == "system.hibernate" {
                let (can, note) = crate::actuators::hibernate_capability();
                if !can {
                    return Some(Response {
                        id: req.id,
                        ok: true,
                        error: None,
                        result: json!({
                            "opened": false,
                            "absent": true,
                            "note": note,
                            "honesty": "sem card hibernate se CanHibernate/NOSD_HIBERNATE falha",
                        }),
                    });
                }
            }
            let gate_hitl =
                nos_core::CapGate::new(nos_core::CapBits::HITL, state.ipc_generation());
            match improve_propose_open(
                &state.hermes.windows,
                &gate_hitl,
                state.ipc_generation(),
                p,
                &state.hermes.improve_fp,
                DEFAULT_FP_TTL_MS,
            ) {
                Ok(out) => Response {
                    id: req.id,
                    ok: true,
                    error: None,
                    result: serde_json::to_value(out).unwrap_or(json!(null)),
                },
                Err(e) => Response {
                    id: req.id,
                    ok: false,
                    error: Some(format!("improve.propose: {e}")),
                    result: json!(null),
                },
            }
        }
        _ => return None,
    })
}
