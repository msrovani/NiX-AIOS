//! LatentBus — canal [f16; LATENT_DIM] paralelo ao EventBus (#444).
//! UI/Orb consome sem match de string.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const LATENT_DIM: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatentFrame {
    /// Valores f32 (host); wire futuro f16.
    pub v: Vec<f32>,
    pub tick: u64,
    pub tag: String,
}

impl LatentFrame {
    pub fn zeros(tag: impl Into<String>, tick: u64) -> Self {
        Self {
            v: vec![0.0; LATENT_DIM],
            tick,
            tag: tag.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct LatentBus {
    latest: Arc<Mutex<Option<LatentFrame>>>,
}

impl LatentBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, frame: LatentFrame) {
        let mut g = self.latest.lock();
        *g = Some(frame);
    }

    pub fn drain(&self) -> Option<LatentFrame> {
        self.latest.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_drain() {
        let b = LatentBus::new();
        let mut f = LatentFrame::zeros("emotion", 1);
        f.v[0] = 0.5;
        b.publish(f);
        let d = b.drain().unwrap();
        assert!((d.v[0] - 0.5).abs() < 1e-6);
    }
}
