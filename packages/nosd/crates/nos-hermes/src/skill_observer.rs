//! SkillObserver — fila fina de nomes vindos de Seek hits (A.1).

use parking_lot::Mutex;

#[derive(Debug, Default)]
pub struct SkillObserver {
    pending_skills: Mutex<Vec<String>>,
}

impl SkillObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, name: impl Into<String>) {
        self.pending_skills.lock().push(name.into());
    }

    pub fn pending(&self) -> Vec<String> {
        self.pending_skills.lock().clone()
    }

    pub fn take_pending(&self) -> Vec<String> {
        std::mem::take(&mut *self.pending_skills.lock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_take() {
        let o = SkillObserver::new();
        o.push("skill-a");
        assert_eq!(o.pending(), vec!["skill-a"]);
        assert!(o.take_pending().len() == 1);
        assert!(o.pending().is_empty());
    }
}
