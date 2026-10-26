//! Conector Cortex → UI: sugere *kind* de janela JARBAS (não desenha pixels).
//! Hermes materializa via WindowBroker; JARBAS renderiza.

use nos_core::WindowKind;

/// Inferência leve de intenção de UI a partir do prompt (keyword; ≠ BitNet).
pub fn suggest_window_kind(prompt: &str) -> Option<WindowKind> {
    let p = prompt.to_ascii_lowercase();
    if p.contains("wifi")
        || p.contains("wi-fi")
        || p.contains("rede sem fio")
        || p.contains("ssid")
        || (p.contains("senha") && (p.contains("rede") || p.contains("wifi") || p.contains("wlan")))
    {
        return Some(WindowKind::HitlWifi);
    }
    if p.contains("aprovar")
        || p.contains("confirmar")
        || p.contains("hitl")
        || p.contains("escalate")
        || p.contains("nix.propose")
        || p.contains("rebuild")
    {
        return Some(WindowKind::HitlConfirm);
    }
    if p.contains("mostrar status") || p.contains("sysinfo") {
        return Some(WindowKind::Status);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_hint() {
        assert_eq!(
            suggest_window_kind("liga o wifi da casa"),
            Some(WindowKind::HitlWifi)
        );
    }

    #[test]
    fn none_for_chat() {
        assert!(suggest_window_kind("oi jarbas").is_none());
    }
}
