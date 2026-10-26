//! BEI tick fino — budget + affect (PORT ADR-0060; não substitui LLM).
//! SleepCycle 5 fases (PORT thin) — REPLAY→DREAM→CONSOLIDATE→PRUNE→REFLECT.

pub mod persona;

pub use persona::{Emotion, PersonaState};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SleepPhase {
    Replay = 0,
    Dream = 1,
    Consolidate = 2,
    Prune = 3,
    Reflect = 4,
}

/// Cinco fases nomeadas (paridade SleepCycleAgent bare-metal).
pub const SLEEP_PHASE_NAMES: [&'static str; 5] =
    ["REPLAY", "DREAM", "CONSOLIDATE", "PRUNE", "REFLECT"];

impl SleepPhase {
    pub fn next(self) -> Self {
        match self {
            Self::Replay => Self::Dream,
            Self::Dream => Self::Consolidate,
            Self::Consolidate => Self::Prune,
            Self::Prune => Self::Reflect,
            Self::Reflect => Self::Replay,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Replay => "REPLAY",
            Self::Dream => "DREAM",
            Self::Consolidate => "CONSOLIDATE",
            Self::Prune => "PRUNE",
            Self::Reflect => "REFLECT",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectSnapshot {
    pub valence: i8,
    pub arousal: i8,
    pub budget_remaining: u32,
}

#[derive(Clone)]
pub struct BeiRuntime {
    inner: Arc<Mutex<BeiState>>,
}

struct BeiState {
    budget: u32,
    valence: i8,
    arousal: i8,
    ticks: u64,
    sleep: SleepPhase,
    /// Avança fase a cada N ticks (default 32 — PollEvery-like).
    sleep_every: u64,
}

impl Default for BeiRuntime {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl BeiRuntime {
    pub fn new(budget: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BeiState {
                budget,
                valence: 0,
                arousal: 0,
                ticks: 0,
                sleep: SleepPhase::Replay,
                sleep_every: 32,
            })),
        }
    }

    /// Tick contínuo com gate de budget (fail-closed quando 0).
    pub fn tick(&self) -> Option<AffectSnapshot> {
        let mut g = self.inner.lock();
        if g.budget == 0 {
            debug!(target: "nosd", "bei tick skip budget=0");
            return None;
        }
        g.budget -= 1;
        g.ticks += 1;
        if g.ticks % 8 == 0 {
            g.arousal = g.arousal.saturating_sub(1);
        }
        if g.sleep_every > 0 && g.ticks % g.sleep_every == 0 {
            g.sleep = g.sleep.next();
            debug!(
                target: "nosd",
                "sleep_cycle phase={} phases={:?}",
                g.sleep.name(),
                SLEEP_PHASE_NAMES
            );
        }
        Some(AffectSnapshot {
            valence: g.valence,
            arousal: g.arousal,
            budget_remaining: g.budget,
        })
    }

    pub fn sleep_phase(&self) -> SleepPhase {
        self.inner.lock().sleep
    }

    pub fn note_event(&self, delta_valence: i8, cost: u32) {
        let mut g = self.inner.lock();
        g.valence = g.valence.saturating_add(delta_valence);
        g.arousal = g.arousal.saturating_add(1);
        g.budget = g.budget.saturating_sub(cost);
    }

    pub fn snapshot(&self) -> AffectSnapshot {
        let g = self.inner.lock();
        AffectSnapshot {
            valence: g.valence,
            arousal: g.arousal,
            budget_remaining: g.budget,
        }
    }

    /// Tick explícito do SleepCycle: loga fase atual, chama `on_consolidate` em CONSOLIDATE, avança fase.
    pub fn tick_sleep_cycle<F>(&self, mut on_consolidate: F) -> SleepPhase
    where
        F: FnMut(SleepPhase),
    {
        let mut g = self.inner.lock();
        let phase = g.sleep;
        debug!(target: "nosd", "sleep_cycle tick phase={}", phase.name());
        if phase == SleepPhase::Consolidate {
            on_consolidate(phase);
        }
        g.sleep = phase.next();
        g.sleep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_exhausts() {
        let b = BeiRuntime::new(2);
        assert!(b.tick().is_some());
        assert!(b.tick().is_some());
        assert!(b.tick().is_none());
    }

    #[test]
    fn sleep_phase_names_five() {
        assert_eq!(SLEEP_PHASE_NAMES.len(), 5);
        assert_eq!(SLEEP_PHASE_NAMES[0], "REPLAY");
        assert_eq!(SLEEP_PHASE_NAMES[4], "REFLECT");
    }

    #[test]
    fn sleep_advances() {
        let b = BeiRuntime::new(100);
        assert_eq!(b.sleep_phase(), SleepPhase::Replay);
        for _ in 0..32 {
            let _ = b.tick();
        }
        assert_eq!(b.sleep_phase(), SleepPhase::Dream);
    }

    #[test]
    fn tick_sleep_cycle_consolidate_callback() {
        let b = BeiRuntime::new(100);
        let mut consolidated = false;
        for _ in 0..8 {
            b.tick_sleep_cycle(|p| {
                if p == SleepPhase::Consolidate {
                    consolidated = true;
                }
            });
            if consolidated {
                break;
            }
        }
        assert!(consolidated);
    }
}
