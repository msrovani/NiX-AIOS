//! V0 (plano vampiro) — pesos do score declarados, não escondidos.
//!
//! Origem da ideia: `configs/openjarvis/config.toml` → `[learning.metrics]`
//! (`accuracy/latency/cost/efficiency`). Aqui os pesos são um contrato puro:
//! parse, validação e normalização. Quem os aplica (`score_v0`, `brain.plan`)
//! lê-os do env `NOSD_SCORE_WEIGHTS` — a decisão fica visível e auditável.

use serde::{Deserialize, Serialize};

/// Pesos por omissão (soma = 1.0). Mudar isto é uma decisão de produto.
pub const DEFAULT_WEIGHTS: ScoreWeights = ScoreWeights {
    accuracy: 0.6,
    latency: 0.2,
    cost: 0.1,
    efficiency: 0.1,
};

/// Peso relativo de cada eixo do score.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoreWeights {
    pub accuracy: f64,
    pub latency: f64,
    pub cost: f64,
    pub efficiency: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        DEFAULT_WEIGHTS
    }
}

impl ScoreWeights {
    /// Soma dos pesos (antes de normalizar).
    pub fn sum(&self) -> f64 {
        self.accuracy + self.latency + self.cost + self.efficiency
    }

    /// Válido = todos finitos e não negativos, soma > 0.
    pub fn is_valid(&self) -> bool {
        let all = [self.accuracy, self.latency, self.cost, self.efficiency];
        all.iter().all(|v| v.is_finite() && *v >= 0.0) && self.sum() > 0.0
    }

    /// Normaliza para soma 1.0 (falha = devolve os defaults).
    pub fn normalized(&self) -> Self {
        if !self.is_valid() {
            return DEFAULT_WEIGHTS;
        }
        let s = self.sum();
        Self {
            accuracy: self.accuracy / s,
            latency: self.latency / s,
            cost: self.cost / s,
            efficiency: self.efficiency / s,
        }
    }

    /// Parse `"0.6,0.2,0.1,0.1"` (accuracy, latency, cost, efficiency).
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(
                "NOSD_SCORE_WEIGHTS: esperados 4 valores (accuracy,latency,cost,efficiency)",
            );
        }
        let mut v = [0.0f64; 4];
        for (i, p) in parts.iter().enumerate() {
            v[i] = p
                .parse::<f64>()
                .map_err(|_| "NOSD_SCORE_WEIGHTS: valor não numérico")?;
        }
        let w = Self {
            accuracy: v[0],
            latency: v[1],
            cost: v[2],
            efficiency: v[3],
        };
        if !w.is_valid() {
            return Err("NOSD_SCORE_WEIGHTS: pesos negativos ou soma zero");
        }
        Ok(w.normalized())
    }

    /// Lê `NOSD_SCORE_WEIGHTS`; erro devolve o motivo para o `config.show`.
    pub fn from_env() -> (Self, Option<String>) {
        match std::env::var("NOSD_SCORE_WEIGHTS") {
            Ok(raw) if !raw.trim().is_empty() => match Self::parse(&raw) {
                Ok(w) => (w, None),
                Err(e) => (DEFAULT_WEIGHTS, Some(format!("{e} (raw=\"{raw}\")"))),
            },
            _ => (DEFAULT_WEIGHTS, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_sum_to_one() {
        let w = ScoreWeights::default();
        assert!((w.sum() - 1.0).abs() < 1e-9);
        assert!(w.is_valid());
    }

    #[test]
    fn parse_and_normalize() {
        let w = ScoreWeights::parse("6,2,1,1").expect("parse");
        assert!((w.sum() - 1.0).abs() < 1e-9);
        assert!((w.accuracy - 0.6).abs() < 1e-9);
        assert!((w.efficiency - 0.1).abs() < 1e-9);
    }

    #[test]
    fn parse_rejects_garbage_and_negatives() {
        assert!(ScoreWeights::parse("0.6,0.2").is_err());
        assert!(ScoreWeights::parse("a,b,c,d").is_err());
        assert!(ScoreWeights::parse("-1,0,0,0").is_err());
        assert!(ScoreWeights::parse("0,0,0,0").is_err());
    }

    #[test]
    fn invalid_normalizes_to_defaults() {
        let bad = ScoreWeights {
            accuracy: -2.0,
            ..DEFAULT_WEIGHTS
        };
        assert_eq!(bad.normalized(), DEFAULT_WEIGHTS);
    }
}
