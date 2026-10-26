//! IterationBudget (P.1) — cap fino de voltas Observe→Act no path chat/Hermes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct IterationBudget {
    pub max: u32,
    pub remaining: u32,
}

impl IterationBudget {
    pub fn new(max: u32) -> Self {
        Self {
            max,
            remaining: max,
        }
    }

    /// Consome uma iteração; `false` = budget esgotado (fail-closed).
    pub fn consume(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }

    pub fn exhausted(&self) -> bool {
        self.remaining == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_until_empty() {
        let mut b = IterationBudget::new(2);
        assert!(b.consume());
        assert!(b.consume());
        assert!(!b.consume());
    }
}
