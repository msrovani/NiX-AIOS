//! WindowBroker — Hermes orquestra janelas; JARBAS materializa; Cortex só sugere.
//! CAP 16 slots; FIFO evict do mais antigo.

use nos_bus::EventBus;
use nos_core::ui_window::topics;
use nos_core::{
    CapBits, CapGate, UiSource, WindowAction, WindowActionKind, WindowKind, WindowSpec,
};
use parking_lot::Mutex;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::info;

const CAP: usize = 16;

#[derive(Clone)]
pub struct WindowBroker {
    open: Arc<Mutex<VecDeque<WindowSpec>>>,
    next_id: Arc<AtomicU64>,
    bus: EventBus,
}

impl WindowBroker {
    pub fn new(bus: EventBus) -> Self {
        Self {
            open: Arc::new(Mutex::new(VecDeque::with_capacity(CAP))),
            next_id: Arc::new(AtomicU64::new(1)),
            bus,
        }
    }

    pub fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Abre janela (mostrar é livre; **acção** HITL exige CapBits::HITL).
    pub fn open(
        &self,
        _gate: &CapGate,
        _cap_gen: u64,
        mut spec: WindowSpec,
    ) -> Result<WindowSpec, &'static str> {
        if spec.id == 0 {
            spec.id = self.alloc_id();
        }
        {
            let mut q = self.open.lock();
            while q.len() >= CAP {
                if let Some(old) = q.pop_front() {
                    self.bus.publish(
                        topics::UI_CLOSE,
                        json!({ "id": old.id, "reason": "cap_evict" }),
                    );
                }
            }
            q.push_back(spec.clone());
        }
        info!(
            target: "nosd",
            "ui open id={} kind={:?} source={:?} escalate={}",
            spec.id,
            spec.kind,
            spec.source,
            spec.escalate
        );
        self.bus
            .publish(topics::UI_OPEN, serde_json::to_value(&spec).unwrap_or(json!({})));
        Ok(spec)
    }

    pub fn list(&self) -> Vec<WindowSpec> {
        self.open.lock().iter().cloned().collect()
    }

    pub fn close(&self, id: u64) -> bool {
        let mut q = self.open.lock();
        let before = q.len();
        q.retain(|w| w.id != id);
        let removed = q.len() != before;
        if removed {
            self.bus
                .publish(topics::UI_CLOSE, json!({ "id": id, "reason": "close" }));
        }
        removed
    }

    /// JARBAS devolve acção HITL → EventBus; fecha janela em Submit/Cancel/Dismiss.
    pub fn action(
        &self,
        gate: &CapGate,
        cap_gen: u64,
        act: WindowAction,
    ) -> Result<serde_json::Value, &'static str> {
        gate.allow(CapBits::HITL, cap_gen)?;
        let spec = {
            let q = self.open.lock();
            q.iter().find(|w| w.id == act.window_id).cloned()
        };
        let Some(spec) = spec else {
            return Err("window not found");
        };
        // Wi‑Fi: não logar password em claro
        let mut safe_values = act.values.clone();
        if safe_values.contains_key("password") {
            safe_values.insert("password".into(), json!("***"));
        }
        let payload = json!({
            "window_id": act.window_id,
            "kind": spec.kind,
            "action": act.action,
            "values": safe_values,
            "source": spec.source,
            "escalate": spec.escalate,
        });
        self.bus.publish(topics::UI_ACTION, payload.clone());
        let reply_topic = if spec.topic_reply.is_empty() {
            topics::UI_HITL_RESULT
        } else {
            spec.topic_reply.as_str()
        };
        self.bus.publish(reply_topic, payload.clone());
        info!(
            target: "nosd",
            "ui action id={} {:?} (password redacted in log path)",
            act.window_id,
            act.action
        );
        match act.action {
            WindowActionKind::Submit | WindowActionKind::Cancel | WindowActionKind::Dismiss => {
                let _ = self.close(act.window_id);
            }
        }
        // applied=false aqui; nosd server pode aplicar nmcli e sobrescrever
        Ok(json!({
            "accepted": true,
            "applied": false,
            "reason": "hitl_accepted_pending_backend",
            "kind": format!("{:?}", spec.kind),
            "result": payload,
        }))
    }

    /// Atalhos de cerimónia. `ssids` vazio → placeholder.
    pub fn open_wifi(
        &self,
        gate: &CapGate,
        cap_gen: u64,
        source: UiSource,
        ssids: Vec<String>,
    ) -> Result<WindowSpec, &'static str> {
        let id = self.alloc_id();
        let list = if ssids.is_empty() {
            vec!["(sem redes — nmcli/scan)".into()]
        } else {
            ssids
        };
        self.open(gate, cap_gen, WindowSpec::hitl_wifi(id, list, source))
    }

    pub fn open_wifi_stub(
        &self,
        gate: &CapGate,
        cap_gen: u64,
        source: UiSource,
    ) -> Result<WindowSpec, &'static str> {
        self.open_wifi(gate, cap_gen, source, try_nmcli_ssids(16))
    }

    pub fn open_from_kind(
        &self,
        gate: &CapGate,
        cap_gen: u64,
        kind: WindowKind,
        source: UiSource,
        title: Option<String>,
        body: Option<String>,
    ) -> Result<WindowSpec, &'static str> {
        let id = self.alloc_id();
        let spec = match kind {
            WindowKind::HitlWifi => {
                return self.open_wifi_stub(gate, cap_gen, source);
            }
            WindowKind::HitlConfirm => WindowSpec::hitl_confirm(
                id,
                title.unwrap_or_else(|| "Confirmar".into()),
                body.unwrap_or_else(|| "HITL JARBAS — confirme ou cancele.".into()),
                source,
            ),
            WindowKind::Info | WindowKind::Status => WindowSpec::info(
                id,
                title.unwrap_or_else(|| "JARBAS".into()),
                body.unwrap_or_else(|| "Status".into()),
                source,
            ),
            WindowKind::HitlText | WindowKind::Custom => {
                let mut w = WindowSpec::info(
                    id,
                    title.unwrap_or_else(|| "Entrada".into()),
                    body.unwrap_or_default(),
                    source,
                );
                w.kind = kind;
                w.escalate = true;
                w.fields = vec![nos_core::HitlField {
                    name: "text".into(),
                    label: "Texto".into(),
                    kind: "text".into(),
                    options: vec![],
                    secret: false,
                    required: true,
                }];
                w
            }
        };
        self.open(gate, cap_gen, spec)
    }
}

fn try_nmcli_ssids(limit: usize) -> Vec<String> {
    use std::process::Command;
    let _ = Command::new("nmcli").args(["device", "wifi", "rescan"]).output();
    let out = match Command::new("nmcli")
        .args(["-t", "-f", "SSID", "device", "wifi", "list"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut v = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let ssid = line.trim();
        if ssid.is_empty() || !seen.insert(ssid.to_string()) {
            continue;
        }
        v.push(ssid.to_string());
        if v.len() >= limit {
            break;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use nos_core::CapBits;

    #[test]
    fn open_list_action() {
        let bus = EventBus::new(16);
        let b = WindowBroker::new(bus);
        let g = CapGate::new(CapBits::HITL, 1);
        let w = b.open_wifi_stub(&g, 1, UiSource::Hermes).unwrap();
        assert_eq!(b.list().len(), 1);
        let act = WindowAction {
            window_id: w.id,
            action: WindowActionKind::Cancel,
            values: Default::default(),
        };
        assert!(b.action(&g, 1, act).unwrap()["accepted"].as_bool().unwrap());
        assert!(b.list().is_empty());
    }
}
