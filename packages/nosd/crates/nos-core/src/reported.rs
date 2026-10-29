//! V8.2 (plano vampiro) — uma shape para cada percentagem que o daemon
//! reporta sobre si próprio.
//!
//! Origem: `omh_reported_rate/v1` (oh-my-hermes). Uma percentagem sem
//! denominador é uma afirmação que ninguém pode verificar; e um denominador
//! vazio **não** é 0% — é "não medido". Aqui:
//!
//! - `denominator == 0` ⇒ `percent = None` + `basis = no_observations`;
//! - numerador composto tem de nomear os baldes somados (`numerator_of`);
//! - `meets_target` **falha** uma rate não medida (não a trata como 0%).

use serde::{Deserialize, Serialize};

pub const REPORTED_RATE_SCHEMA_VERSION: &str = "nosd_reported_rate/v1";
pub const BASIS_OBSERVED: &str = "observed_cases";
pub const BASIS_NONE: &str = "no_observations";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportedRate {
    pub schema_version: String,
    /// `None` quando o denominador é 0 (nunca `0.0`).
    pub percent: Option<f64>,
    pub numerator: u64,
    pub denominator: u64,
    /// Baldes somados no numerador (nunca vazio).
    pub numerator_of: Vec<String>,
    /// O que o denominador contou.
    pub denominator_of: String,
    /// Classes excluídas antes de contar.
    pub excluded: Vec<String>,
    pub basis: String,
}

/// Constrói uma rate que carrega o que dividiu.
pub fn reported_rate(
    numerator: u64,
    denominator: u64,
    numerator_of: &[&str],
    denominator_of: &str,
    excluded: &[&str],
) -> Result<ReportedRate, &'static str> {
    if numerator > denominator {
        return Err("reported_rate: numerator > denominator");
    }
    let names: Vec<String> = numerator_of
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Err("reported_rate: numerator_of vazio (nomeie o que foi contado)");
    }
    let den = denominator_of.trim();
    if den.is_empty() {
        return Err("reported_rate: denominator_of vazio");
    }
    let basis = if denominator == 0 {
        BASIS_NONE
    } else {
        BASIS_OBSERVED
    };
    let percent = if denominator == 0 {
        None
    } else {
        Some(round1(numerator as f64 * 100.0 / denominator as f64))
    };
    Ok(ReportedRate {
        schema_version: REPORTED_RATE_SCHEMA_VERSION.to_string(),
        percent,
        numerator,
        denominator,
        numerator_of: names,
        denominator_of: den.to_string(),
        excluded: excluded
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        basis: basis.to_string(),
    })
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

impl ReportedRate {
    /// Uma rate não medida **não** cumpre a meta.
    pub fn meets_target(&self, target_percent: f64) -> bool {
        self.percent.map(|p| p >= target_percent).unwrap_or(false)
    }

    /// Linha que carrega o denominador.
    pub fn line(&self) -> String {
        let head = match self.percent {
            Some(p) => format!("{p}%"),
            None => format!("não medido ({})", self.basis),
        };
        let mut s = format!(
            "{head} — {}/{} {} ({})",
            self.numerator,
            self.denominator,
            self.denominator_of,
            self.numerator_of.join(" + ")
        );
        if !self.excluded.is_empty() {
            s.push_str(&format!("; excluído: {}", self.excluded.join(", ")));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_denominator_is_unmeasured_not_zero() {
        let r = reported_rate(0, 0, &["turns_with_energy"], "chat_turns", &[]).unwrap();
        assert_eq!(r.percent, None);
        assert_eq!(r.basis, BASIS_NONE);
        assert!(!r.meets_target(0.0), "não medido nunca cumpre a meta");
        assert!(r.line().contains("não medido"));
    }

    #[test]
    fn observed_rate_carries_denominator_and_rounds() {
        let r = reported_rate(2, 3, &["turns_with_energy"], "chat_turns", &[]).unwrap();
        assert_eq!(r.percent, Some(66.7));
        assert_eq!(r.basis, BASIS_OBSERVED);
        assert!(r.meets_target(60.0));
        assert!(!r.meets_target(70.0));
        assert!(r.line().contains("2/3 chat_turns"));
    }

    #[test]
    fn composed_numerator_names_every_bucket() {
        let r = reported_rate(
            3,
            4,
            &["rapl", "nvml", "tdp_estimate"],
            "chat_turns",
            &["s2_pending"],
        )
        .unwrap();
        assert_eq!(r.numerator_of.len(), 3);
        assert!(r.line().contains("rapl + nvml + tdp_estimate"));
        assert!(r.line().contains("excluído: s2_pending"));
    }

    #[test]
    fn invalid_inputs_fail_closed() {
        assert!(reported_rate(3, 2, &["a"], "d", &[]).is_err());
        assert!(reported_rate(1, 2, &[], "d", &[]).is_err());
        assert!(reported_rate(1, 2, &["  "], "d", &[]).is_err());
        assert!(reported_rate(1, 2, &["a"], "   ", &[]).is_err());
    }

    #[test]
    fn payload_is_self_describing() {
        let r = reported_rate(1, 2, &["a"], "d", &["x"]).unwrap();
        let v = serde_json::to_value(&r).unwrap();
        for k in [
            "schema_version",
            "percent",
            "numerator",
            "denominator",
            "numerator_of",
            "denominator_of",
            "excluded",
            "basis",
        ] {
            assert!(v.get(k).is_some(), "falta {k}");
        }
        assert_eq!(v["percent"], 50.0);
    }
}
