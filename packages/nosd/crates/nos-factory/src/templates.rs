//! Factory templates (CHJ phase 2–3): WASM placeholder com JSON fixo no buffer ABI v2.
//! Documentado como template — não codegen livre.

use crate::content_id;
use nos_core::CapBits;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateId {
    NetMonitor,
    LogHeartbeat,
    EchoInfer,
}

impl TemplateId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NetMonitor => "net.monitor",
            Self::LogHeartbeat => "log.heartbeat",
            Self::EchoInfer => "echo.infer",
        }
    }

    pub fn caps(self) -> Vec<&'static str> {
        match self {
            // Host ALLOWED_CAPS = aios_ai.infer|log; telemetria de rede fica
            // no host (`observe.net`), não como cap wasm inventada.
            Self::NetMonitor => vec!["log"],
            Self::LogHeartbeat => vec!["log"],
            Self::EchoInfer => vec!["aios_ai.infer", "log"],
        }
    }
}

fn template_json_wasm(json: &str) -> Vec<u8> {
    let escaped = json.replace('\\', "\\\\").replace('"', "\\\"");
    let len = json.len();
    let wat = format!(
        r#"(module
            (memory (export "memory") 1)
            (data (i32.const 64) "{escaped}")
            (func (export "run") (param i32) (result i32)
                local.get 0)
            (func (export "result_ptr") (result i32) (i32.const 64))
            (func (export "result_len") (result i32) (i32.const {len})))"#
    );
    wat::parse_str(&wat).expect("template wat")
}

fn net_monitor_wasm() -> &'static [u8] {
    static W: OnceLock<Vec<u8>> = OnceLock::new();
    W.get_or_init(|| template_json_wasm(r#"{"template":"net.monitor","status":"stub"}"#))
}

fn log_heartbeat_wasm() -> &'static [u8] {
    static W: OnceLock<Vec<u8>> = OnceLock::new();
    W.get_or_init(|| template_json_wasm(r#"{"template":"log.heartbeat","beat":"stub"}"#))
}

fn echo_infer_wasm() -> &'static [u8] {
    static W: OnceLock<Vec<u8>> = OnceLock::new();
    W.get_or_init(|| template_json_wasm(r#"{"template":"echo.infer","echo":"stub"}"#))
}

/// Bytes WASM do template (ABI v2, resultado JSON fixo).
pub fn template_wasm(id: TemplateId) -> Vec<u8> {
    match id {
        TemplateId::NetMonitor => net_monitor_wasm().to_vec(),
        TemplateId::LogHeartbeat => log_heartbeat_wasm().to_vec(),
        TemplateId::EchoInfer => echo_infer_wasm().to_vec(),
    }
}

/// Escolhe template por palavras-chave no intent (pt/en).
pub fn pick_template_id(intent: &str) -> TemplateId {
    let s = intent.to_lowercase();
    if s.contains("heartbeat") || s.contains("log") && s.contains("pulse") {
        return TemplateId::LogHeartbeat;
    }
    if s.contains("infer") || s.contains("echo") || s.contains("llm") {
        return TemplateId::EchoInfer;
    }
    if s.contains("rede")
        || s.contains("network")
        || s.contains("monitor")
        || s.contains("monitorar")
        || s.contains("net.")
    {
        return TemplateId::NetMonitor;
    }
    TemplateId::EchoInfer
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoryPlanResult {
    pub template_id: String,
    pub content_id: String,
    pub caps: Vec<String>,
    pub note: String,
    pub seek_before: Option<String>,
}

/// `factory.plan`: intent → template + BLAKE3; nota seek-before se caller passar report.
pub fn factory_plan(intent: &str, seek_note: Option<&str>) -> FactoryPlanResult {
    let tid = pick_template_id(intent);
    let wasm = template_wasm(tid);
    let cid = content_id(&wasm);
    let mut note = format!(
        "template {} — WASM placeholder (JSON fixo); validar no sandbox antes de skill.install",
        tid.as_str()
    );
    if let Some(s) = seek_note {
        note.push_str("; seek-before: ");
        note.push_str(s);
    }
    FactoryPlanResult {
        template_id: tid.as_str().to_string(),
        content_id: cid,
        caps: tid.caps().into_iter().map(str::to_string).collect(),
        note,
        seek_before: seek_note.map(str::to_string),
    }
}

/// Intent de chat: criar skill/app + monitor de rede (pt/en).
/// Também aceita coloquial: “app de monitoramento da rede” sem verbo criar.
pub fn match_skill_create_network_intent(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();
    let create = [
        "criar",
        "crie",
        "create",
        "nova skill",
        "new skill",
        "quero uma skill",
        "quero um app",
        "make a skill",
        "build a skill",
        "gerar skill",
        "gerar app",
        "faz um app",
        "faz uma skill",
        "monta um app",
    ];
    let network = [
        "rede",
        "network",
        "monitor",
        "monitorar",
        "monitoramento",
        "monitoring",
        "tráfego",
        "trafego",
        "traffic",
        "net monitor",
    ];
    let appish = lower.contains("app") || lower.contains("skill") || lower.contains("aplicativo");
    let has_net = network.iter().any(|k| lower.contains(k));
    let has_create = create.iter().any(|k| lower.contains(k));
    if !has_net {
        return None;
    }
    if !(has_create || (appish && (lower.contains("monitor") || lower.contains("rede")))) {
        return None;
    }
    Some(prompt.trim().to_string())
}

/// Draft pronto para promote/install a partir do plan.
pub fn draft_from_plan(plan: &FactoryPlanResult) -> crate::ArtifactDraft {
    let tid = pick_template_id(&plan.template_id);
    crate::ArtifactDraft {
        name: plan.template_id.replace('.', "-"),
        wasm: template_wasm(tid),
        path: crate::FactoryPath::AWasmi,
    }
}

/// CapBits mínimas sugeridas para validar o template no sandbox.
pub fn cap_bits_for_template(id: TemplateId) -> CapBits {
    let mut c = CapBits::NONE;
    for cap in id.caps() {
        match cap {
            "log" => c = c.union(CapBits::MEMORY),
            "net:read" => c = c.union(CapBits::NET),
            "aios_ai.infer" => c = c.union(CapBits::INFER),
            _ => {}
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_net_monitor_keywords() {
        assert_eq!(
            pick_template_id("monitorar rede wifi"),
            TemplateId::NetMonitor
        );
        assert_eq!(
            pick_template_id("network monitor skill"),
            TemplateId::NetMonitor
        );
    }

    #[test]
    fn intent_match_pt_en() {
        assert!(
            match_skill_create_network_intent("Crie uma skill para monitorar a rede").is_some()
        );
        assert!(match_skill_create_network_intent("create network monitor app").is_some());
        assert!(match_skill_create_network_intent("olá mundo").is_none());
        assert!(match_skill_create_network_intent("criar skill de log").is_none());
    }

    #[test]
    fn factory_plan_content_id_stable() {
        let a = factory_plan("rede monitor", None);
        let b = factory_plan("rede monitor", None);
        assert_eq!(a.content_id, b.content_id);
        assert_eq!(a.template_id, "net.monitor");
    }
}
