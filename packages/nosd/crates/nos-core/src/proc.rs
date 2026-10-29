//! D1 (plano vampiro) — allowlist de binários externos consultados pelo daemon.
//!
//! Regra: **o `PATH` não decide**. Só caminhos absolutos, numa lista fechada e
//! determinística. Um override por env só é aceite se for absoluto; relativo é
//! ignorado (fail-closed), para que um `PATH`/cwd manipulado não troque o
//! binário executado.
//!
//! Origem do padrão: `PROCESS_SPAWN_ALLOWLIST` (oh-my-hermes).
//! Inventário completo: `docs/SPAWN_INVENTORY.md`.

use std::path::PathBuf;

/// Caminho canónico no NixOS (perfil do sistema independente do `path` do unit).
pub const NVIDIA_SMI_NIXOS: &str = "/run/current-system/sw/bin/nvidia-smi";

/// Candidatos absolutos, por ordem de preferência.
pub const NVIDIA_SMI_CANDIDATES: &[&str] = &[
    NVIDIA_SMI_NIXOS,
    "/usr/bin/nvidia-smi",
    "/usr/local/bin/nvidia-smi",
    r"C:\Windows\System32\nvidia-smi.exe",
    r"C:\Program Files\NVIDIA Corporation\NVSMI\nvidia-smi.exe",
];

/// Lista de candidatos com override explícito (`NOSD_NVIDIA_SMI`).
///
/// - override absoluto e não vazio → **só** esse (intenção explícita);
/// - override ausente/relativo → lista por omissão.
pub fn nvidia_smi_candidates_with(override_path: Option<&str>) -> Vec<PathBuf> {
    if let Some(raw) = override_path {
        let raw = raw.trim();
        if !raw.is_empty() {
            let p = PathBuf::from(raw);
            if p.is_absolute() {
                return vec![p];
            }
        }
    }
    NVIDIA_SMI_CANDIDATES.iter().map(PathBuf::from).collect()
}

/// Lê `NOSD_NVIDIA_SMI` e devolve a lista efectiva.
pub fn nvidia_smi_candidates() -> Vec<PathBuf> {
    let env = std::env::var("NOSD_NVIDIA_SMI").ok();
    nvidia_smi_candidates_with(env.as_deref())
}

/// Primeiro candidato que exista como ficheiro regular.
pub fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

/// Caminho resolvido do `nvidia-smi` (ou `None` se nenhum candidato existir).
pub fn nvidia_smi_path() -> Option<PathBuf> {
    first_existing(&nvidia_smi_candidates())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_list_starts_with_nixos_and_has_no_bare_names() {
        let list = nvidia_smi_candidates_with(None);
        assert_eq!(list[0], PathBuf::from(NVIDIA_SMI_NIXOS));
        // Nenhum nome nu ("nvidia-smi") — o PATH não pode decidir.
        // (Não se usa `is_absolute`: um caminho Unix não é "absoluto" no Windows.)
        assert!(list.iter().all(|p| p.components().count() > 1), "{list:?}");
    }

    #[test]
    fn absolute_override_replaces_the_list() {
        let abs = std::env::temp_dir().join("nv").join("nvidia-smi");
        assert!(abs.is_absolute(), "temp_dir deve ser absoluto: {abs:?}");
        let list = nvidia_smi_candidates_with(Some(abs.to_str().expect("utf8")));
        assert_eq!(list, vec![abs]);
    }

    #[test]
    fn relative_or_empty_override_is_ignored() {
        for bad in ["", "   ", "nvidia-smi", "./nvidia-smi", "../smi"] {
            let list = nvidia_smi_candidates_with(Some(bad));
            assert_eq!(list[0], PathBuf::from(NVIDIA_SMI_NIXOS), "override={bad:?}");
            assert!(list.len() > 1);
        }
    }

    #[test]
    fn first_existing_picks_a_real_file() {
        // Sem `tempfile`: nos-core é crate de contratos (sem dev-deps).
        let dir = std::env::temp_dir().join(format!(
            "nosd-proc-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let a = dir.join("a");
        let b = dir.join("b");
        std::fs::write(&b, b"x").expect("write");
        let list = vec![a, b.clone()];
        assert_eq!(first_existing(&list), Some(b));
        assert_eq!(first_existing(&[]), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolver_never_returns_a_bare_name() {
        for p in nvidia_smi_candidates() {
            assert!(p.components().count() > 1, "{p:?}");
        }
    }
}
