//! Monta prompt de chat: SOUL.md + recall lexical (cognitive bridge).

use nos_memory::NosMemory;
use std::path::{Path, PathBuf};

/// Caminho default do persona SOUL (override via `NOSD_SOUL_PATH`).
pub fn soul_path_default() -> PathBuf {
    std::env::var("NOSD_SOUL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("SOUL.md"))
}

pub fn read_soul_excerpt(path: &Path, max_bytes: usize) -> Option<String> {
    let data = std::fs::read_to_string(path).ok()?;
    let trimmed: String = data.chars().take(max_bytes).collect();
    Some(trimmed)
}

/// Prompt enriquecido: persona + snippets de memória + pedido do utilizador.
pub fn build_chat_prompt(
    memory: &NosMemory,
    user_prompt: &str,
    soul_path: Option<&Path>,
    recall_k: usize,
) -> String {
    let soul_p = soul_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(soul_path_default);
    let soul_block = read_soul_excerpt(&soul_p, 2048)
        .map(|s| format!("## Persona (SOUL)\n{s}\n"))
        .unwrap_or_default();

    let recall_block = memory
        .recall(user_prompt, recall_k)
        .ok()
        .filter(|h| !h.is_empty())
        .map(|hits| {
            let lines: Vec<String> = hits.iter().map(|h| format!("- {}", h.text)).collect();
            format!("## Memory recall\n{}\n", lines.join("\n"))
        })
        .unwrap_or_default();

    format!(
        "{soul_block}{recall_block}## User\n{user_prompt}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nos_memory::{MemoryRecord, NosMemory};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn prompt_includes_soul_and_recall() {
        let d = tempdir().unwrap();
        let soul = d.path().join("SOUL.md");
        let mut f = std::fs::File::create(&soul).unwrap();
        writeln!(f, "Jarvis persona line").unwrap();
        let mem = NosMemory::open(d.path()).unwrap();
        mem.remember(MemoryRecord {
            key: "k1".into(),
            text: "wifi mesh lab".into(),
            scope: "s".into(),
            entities: vec![],
        })
        .unwrap();
        let p = build_chat_prompt(&mem, "wifi help", Some(&soul), 3);
        assert!(p.contains("Jarvis persona"));
        assert!(p.contains("wifi mesh"));
        assert!(p.contains("wifi help"));
    }
}
