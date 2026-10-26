//! Heurística de nome de ficheiro → ModelSlot (scan /var/lib/nosd/models).

use nos_core::ModelSlot;
use std::path::Path;

/// Mapeia path/nome para slot. Fallback Active.
pub fn slot_from_path(path: &Path) -> ModelSlot {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.contains("hw") || name.contains("expert") || name.contains("pci") {
        return ModelSlot::HwExpert;
    }
    if name.contains("rust") || name.contains("coder") || name.contains("code") {
        return ModelSlot::RustCoder;
    }
    if name.contains("vision") || name.contains("siglip") || name.contains("clip") {
        return ModelSlot::Vision;
    }
    if name.contains("rerank") {
        return ModelSlot::Reranker;
    }
    if name.contains("learn") || name.contains("1b") || name.contains("e1b") {
        return ModelSlot::Learner;
    }
    if name.contains("agent") {
        return ModelSlot::Agent;
    }
    if name.contains("pro")
        || name.contains("7b")
        || name.contains("10b")
        || name.contains("70b")
        || name.contains("generator")
    {
        return ModelSlot::GeneratorPro;
    }
    ModelSlot::Active
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn maps_pro() {
        assert_eq!(
            slot_from_path(&PathBuf::from("/m/falcon-pro-7b.gguf")),
            ModelSlot::GeneratorPro
        );
    }

    #[test]
    fn maps_default_active() {
        assert_eq!(
            slot_from_path(&PathBuf::from("/m/model.gguf")),
            ModelSlot::Active
        );
    }
}
