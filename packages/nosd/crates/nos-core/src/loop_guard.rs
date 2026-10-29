//! V8.6 (plano vampiro) — breaker por **hash de artefacto**.
//!
//! Origem: anti-loop `SHA256(git diff)` do eco-tokens. Importa-se a ideia,
//! **não** a acção: aqui nada é revertido. O contrato é:
//!
//! - a chave é um fingerprint determinístico do pedido/artefacto (BLAKE3 no
//!   chamador);
//! - se o **mesmo** fingerprint já falhou, `is_loop` devolve `true` e o
//!   chamador recusa antes de repetir trabalho;
//! - um sucesso limpa o histórico daquele fingerprint.
//!
//! Deliberadamente não há `checkout`/rollback: apagar trabalho do utilizador
//! por causa de um teste flaky é proibido (membrana do plano, §12-D).

use std::collections::HashMap;

/// Estado do breaker (por fingerprint).
#[derive(Debug, Default, Clone)]
pub struct LoopGuard {
    failures: HashMap<String, u32>,
}

impl LoopGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Já falhou pelo menos uma vez com este fingerprint?
    pub fn is_loop(&self, fingerprint: &str) -> bool {
        self.failures(fingerprint) > 0
    }

    pub fn failures(&self, fingerprint: &str) -> u32 {
        self.failures.get(fingerprint).copied().unwrap_or(0)
    }

    /// Registra uma falha. Devolve o total de falhas deste fingerprint.
    pub fn note_failure(&mut self, fingerprint: &str) -> u32 {
        if fingerprint.trim().is_empty() {
            return 0;
        }
        let n = self.failures.entry(fingerprint.to_string()).or_insert(0);
        *n = n.saturating_add(1);
        *n
    }

    /// Registra sucesso: o fingerprint deixa de ser um loop.
    pub fn note_success(&mut self, fingerprint: &str) {
        self.failures.remove(fingerprint);
    }

    /// Nº de fingerprints em falha (diagnóstico/HUD).
    pub fn len(&self) -> usize {
        self.failures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }

    /// Apara fingerprints antigos quando o mapa cresce (limite duro).
    pub fn cap(&mut self, max: usize) {
        if self.failures.len() <= max {
            return;
        }
        let excess = self.failures.len() - max;
        let keys: Vec<String> = self.failures.keys().take(excess).cloned().collect();
        for k in keys {
            self.failures.remove(&k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_fingerprint_after_failure_is_a_loop() {
        let mut g = LoopGuard::new();
        assert!(!g.is_loop("h1"));
        assert_eq!(g.note_failure("h1"), 1);
        assert!(g.is_loop("h1"));
        assert_eq!(g.note_failure("h1"), 2);
    }

    #[test]
    fn success_clears_the_fingerprint() {
        let mut g = LoopGuard::new();
        g.note_failure("h1");
        assert!(g.is_loop("h1"));
        g.note_success("h1");
        assert!(!g.is_loop("h1"));
        assert_eq!(g.failures("h1"), 0);
    }

    #[test]
    fn distinct_fingerprints_do_not_interfere() {
        let mut g = LoopGuard::new();
        g.note_failure("h1");
        assert!(g.is_loop("h1"));
        assert!(!g.is_loop("h2"));
        g.note_failure("h2");
        assert_eq!(g.len(), 2);
        g.note_success("h1");
        assert!(!g.is_loop("h1"));
        assert!(g.is_loop("h2"));
    }

    #[test]
    fn empty_fingerprint_is_never_a_loop() {
        let mut g = LoopGuard::new();
        assert_eq!(g.note_failure(""), 0);
        assert_eq!(g.note_failure("   "), 0);
        assert!(!g.is_loop(""));
        assert!(g.is_empty());
    }

    #[test]
    fn cap_bounds_the_map() {
        let mut g = LoopGuard::new();
        for i in 0..10 {
            g.note_failure(&format!("h{i}"));
        }
        g.cap(4);
        assert_eq!(g.len(), 4);
    }
}
