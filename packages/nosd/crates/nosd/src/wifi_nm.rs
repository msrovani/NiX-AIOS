//! Wi‑Fi via NetworkManager (`nmcli`). CapGate HITL no caller.
//! Honesty: sem NM/radio → Err honesto (não stub de sucesso).

use serde::Serialize;
use std::process::Command;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum WifiError {
    #[error("nmcli absent")]
    NoNmcli,
    #[error("nmcli: {0}")]
    Nm(String),
    #[error("ssid empty")]
    EmptySsid,
}

#[derive(Debug, Clone, Serialize)]
pub struct WifiAp {
    pub ssid: String,
    pub signal: u8,
    pub security: String,
}

fn nmcli() -> Result<&'static str, WifiError> {
    if Command::new("nmcli").arg("--version").output().is_ok() {
        Ok("nmcli")
    } else {
        Err(WifiError::NoNmcli)
    }
}

/// Scan SSIDs (até 24).
pub fn scan(limit: usize) -> Result<Vec<WifiAp>, WifiError> {
    let bin = nmcli()?;
    let _ = Command::new(bin)
        .args(["device", "wifi", "rescan"])
        .output();
    let out = Command::new(bin)
        .args([
            "-t",
            "-f",
            "SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
        ])
        .output()
        .map_err(|e| WifiError::Nm(e.to_string()))?;
    if !out.status.success() {
        return Err(WifiError::Nm(String::from_utf8_lossy(&out.stderr).into()));
    }
    let mut aps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.is_empty() {
            continue;
        }
        let ssid = parts[0].trim().to_string();
        if ssid.is_empty() || !seen.insert(ssid.clone()) {
            continue;
        }
        let signal = parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0u8);
        let security = parts.get(2).unwrap_or(&"").to_string();
        aps.push(WifiAp {
            ssid,
            signal,
            security,
        });
        if aps.len() >= limit {
            break;
        }
    }
    info!(target: "nosd", "wifi scan n={}", aps.len());
    Ok(aps)
}

/// Liga SSID (+ senha opcional para open).
pub fn connect(ssid: &str, password: &str) -> Result<(), WifiError> {
    let ssid = ssid.trim();
    if ssid.is_empty() {
        return Err(WifiError::EmptySsid);
    }
    let bin = nmcli()?;
    let mut cmd = Command::new(bin);
    cmd.args(["device", "wifi", "connect", ssid]);
    if !password.is_empty() {
        cmd.args(["password", password]);
    }
    let out = cmd
        .output()
        .map_err(|e| WifiError::Nm(e.to_string()))?;
    if out.status.success() {
        info!(target: "nosd", "wifi connect ok ssid={}", ssid);
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        warn!(target: "nosd", "wifi connect fail ssid={} err={}", ssid, err.trim());
        Err(WifiError::Nm(err.trim().into()))
    }
}
