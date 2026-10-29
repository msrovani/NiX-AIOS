//! V2 (plano vampiro) — custo local vs contrafactual de nuvem.
//!
//! Origem da ideia: `openjarvis/server/{savings,cost_calculator}.py`.
//!
//! **Honestidade (ADR-0113):** o custo local é derivado de joules medidos (ou
//! `estimate` rotulado); o custo de nuvem é um **contrafactual** e por isso
//! vem sempre com `ref_date` + `ref_model` e o estado `HYPOTHESIS`. Sem esses
//! campos, não se mostra o número.

use serde::{Deserialize, Serialize};

/// 1 kWh em joules.
pub const JOULES_PER_KWH: f64 = 3_600_000.0;

/// Preços de referência por omissão (documentados em `docs/PRICE_REFERENCE.md`).
pub const DEFAULT_KWH_PRICE_EUR: f64 = 0.25;
pub const DEFAULT_CLOUD_USD_IN_PER_MTOK: f64 = 0.60;
pub const DEFAULT_CLOUD_USD_OUT_PER_MTOK: f64 = 2.40;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostModel {
    /// € por kWh (tarifa doméstica média — editável).
    pub kwh_price_eur: f64,
    /// Data a que os preços se referem (`YYYY-MM-DD`).
    pub ref_date: String,
    /// Nome do modelo/API de referência do contrafactual.
    pub ref_model: String,
    pub cloud_in_usd_per_mtok: f64,
    pub cloud_out_usd_per_mtok: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            kwh_price_eur: DEFAULT_KWH_PRICE_EUR,
            ref_date: "2026-10-29".into(),
            ref_model: "cloud API genérica 4B–9B (referência, não benchmark)".into(),
            cloud_in_usd_per_mtok: DEFAULT_CLOUD_USD_IN_PER_MTOK,
            cloud_out_usd_per_mtok: DEFAULT_CLOUD_USD_OUT_PER_MTOK,
        }
    }
}

impl CostModel {
    /// Custo local em euros a partir de joules.
    pub fn local_eur(&self, joules: f64) -> f64 {
        if !joules.is_finite() || joules <= 0.0 {
            return 0.0;
        }
        joules / JOULES_PER_KWH * self.kwh_price_eur
    }

    /// Contrafactual de nuvem em USD (tokens estimados, não medidos).
    pub fn cloud_usd(&self, tokens_in: u64, tokens_out: u64) -> f64 {
        let m_in = tokens_in as f64 / 1_000_000.0;
        let m_out = tokens_out as f64 / 1_000_000.0;
        m_in * self.cloud_in_usd_per_mtok + m_out * self.cloud_out_usd_per_mtok
    }

    /// Carimbo de honestidade que acompanha qualquer valor de nuvem.
    pub fn honesty_tag(&self) -> String {
        format!(
            "HYPOTHESIS ref_date={} ref_model={}",
            self.ref_date, self.ref_model
        )
    }

    /// Lê os overrides de env (best-effort; inválidos são ignorados).
    pub fn from_env() -> Self {
        let mut m = Self::default();
        if let Some(v) = env_f64("NOSD_KWH_PRICE_EUR") {
            m.kwh_price_eur = v;
        }
        if let Some(v) = env_f64("NOSD_CLOUD_USD_IN") {
            m.cloud_in_usd_per_mtok = v;
        }
        if let Some(v) = env_f64("NOSD_CLOUD_USD_OUT") {
            m.cloud_out_usd_per_mtok = v;
        }
        if let Ok(v) = std::env::var("NOSD_PRICE_REF_DATE") {
            if !v.trim().is_empty() {
                m.ref_date = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("NOSD_PRICE_REF_MODEL") {
            if !v.trim().is_empty() {
                m.ref_model = v.trim().to_string();
            }
        }
        m
    }
}

fn env_f64(key: &str) -> Option<f64> {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
}

/// Estimativa de tokens a partir de caracteres (heurística ~4 chars/token).
///
/// **Nunca** é apresentada como contagem do motor — vai sempre com
/// `source = "char_heuristic"`.
pub fn tokens_estimate(chars: usize) -> u64 {
    ((chars as f64) / 4.0).ceil() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_kwh_costs_the_tariff() {
        let m = CostModel::default();
        assert!((m.local_eur(JOULES_PER_KWH) - m.kwh_price_eur).abs() < 1e-9);
        assert_eq!(m.local_eur(0.0), 0.0);
        assert_eq!(m.local_eur(-1.0), 0.0);
    }

    #[test]
    fn cloud_counterfactual_math() {
        let m = CostModel::default();
        let usd = m.cloud_usd(1_000_000, 0);
        assert!((usd - m.cloud_in_usd_per_mtok).abs() < 1e-9);
        let usd2 = m.cloud_usd(0, 1_000_000);
        assert!((usd2 - m.cloud_out_usd_per_mtok).abs() < 1e-9);
        assert_eq!(m.cloud_usd(0, 0), 0.0);
    }

    #[test]
    fn honesty_tag_carries_date_and_model() {
        let m = CostModel::default();
        let tag = m.honesty_tag();
        assert!(tag.starts_with("HYPOTHESIS"));
        assert!(tag.contains(&m.ref_date));
    }

    #[test]
    fn token_estimate_is_labelled_heuristic() {
        assert_eq!(tokens_estimate(0), 0);
        assert_eq!(tokens_estimate(4), 1);
        assert_eq!(tokens_estimate(5), 2);
    }
}
