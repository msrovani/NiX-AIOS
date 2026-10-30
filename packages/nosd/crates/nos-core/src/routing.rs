//! V10 — routing S1/S2 coerente com a infraestrutura real.
//!
//! O `score_intent` responde "que tipo de pergunta é". Este módulo responde
//! "**quem** a pode responder agora" — e é deliberadamente puro: entra o estado
//! observado, sai a decisão + razão. Sem I/O, testável, explicável.
//!
//! Regras (por ordem, fail-closed):
//! 1. lane S1 → S1 (se não há motor, `CpuEcho` honesto).
//! 2. lane S2, mas S2 ausente → S1 (`s2_absent`).
//! 3. `power_save` (presença Sleep/Drowsy) → S1 (`power_save`).
//! 4. prompt acima do contexto útil do S2 → S1 (`prompt_too_long`).
//! 5. interativo com S2 ocupado → S1 (`s2_busy_interactive`) — não bloquear o
//!    chat à espera de uma fila de 4B.
//! 6. caso contrário S2 (`lane_s2`).
//!
//! `CpuEcho` só aparece se nem S1 existe; nunca é escolhido "por gosto".

/// Lane pedida pelo classificador (S1 = rápido/local, S2 = profundo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    S1,
    S2,
}

/// Backend escolhido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    S1,
    S2,
    CpuEcho,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteInputs {
    pub lane: Lane,
    pub prompt_chars: usize,
    /// Chat síncrono (interativo) vs job/assíncrono.
    pub interactive: bool,
    /// `hotswap_s2.is_some()` — o 4B está configurado e anexado.
    pub s2_available: bool,
    /// Pedidos S2 em fila (backpressure).
    pub s2_pending: u32,
    /// Presença Sleep/Drowsy → libertar VRAM.
    pub power_save: bool,
    /// O S1 não é `cpu-echo`.
    pub s1_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    pub backend: Backend,
    /// Razão estável (para `route.explain`/logs).
    pub reason: &'static str,
    /// Houve downgrade face à lane pedida.
    pub downgraded: bool,
}

/// Acima disto o S2 (4k de contexto) não é útil: o prompt é cortado ou o
/// modelo perde o fio. ~6k tokens a 4 chars/token.
pub const MAX_S2_CHARS: usize = 24_000;

/// Presença → poupança (Sleep/Drowsy). Puro para ser testável.
pub fn power_save_from_presence(presence: &str) -> bool {
    matches!(
        presence.trim().to_ascii_lowercase().as_str(),
        "sleep" | "drowsy"
    )
}

/// Lê `NOSD_PRESENCE` (ausente = ativo).
pub fn power_save_from_env() -> bool {
    std::env::var("NOSD_PRESENCE")
        .map(|p| power_save_from_presence(&p))
        .unwrap_or(false)
}

fn s1_backend(i: &RouteInputs, reason: &'static str, downgraded: bool) -> RoutePlan {
    RoutePlan {
        backend: if i.s1_available {
            Backend::S1
        } else {
            Backend::CpuEcho
        },
        reason,
        downgraded,
    }
}

/// Decide o backend para um pedido.
pub fn plan_backend(i: &RouteInputs) -> RoutePlan {
    if i.lane == Lane::S1 {
        return s1_backend(i, "lane_s1", false);
    }
    if !i.s2_available {
        return s1_backend(i, "s2_absent", true);
    }
    if i.power_save {
        return s1_backend(i, "power_save", true);
    }
    if i.prompt_chars > MAX_S2_CHARS {
        return s1_backend(i, "prompt_too_long", true);
    }
    if i.interactive && i.s2_pending > 0 {
        return s1_backend(i, "s2_busy_interactive", true);
    }
    RoutePlan {
        backend: Backend::S2,
        reason: "lane_s2",
        downgraded: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> RouteInputs {
        RouteInputs {
            lane: Lane::S2,
            prompt_chars: 200,
            interactive: true,
            s2_available: true,
            s2_pending: 0,
            power_save: false,
            s1_available: true,
        }
    }

    #[test]
    fn s1_lane_stays_s1() {
        let i = RouteInputs {
            lane: Lane::S1,
            ..base()
        };
        let p = plan_backend(&i);
        assert_eq!(p.backend, Backend::S1);
        assert_eq!(p.reason, "lane_s1");
        assert!(!p.downgraded);
    }

    #[test]
    fn s1_without_engine_is_cpu_echo() {
        let i = RouteInputs {
            lane: Lane::S1,
            s1_available: false,
            ..base()
        };
        assert_eq!(plan_backend(&i).backend, Backend::CpuEcho);
    }

    #[test]
    fn s2_lane_uses_s2_when_everything_ok() {
        let p = plan_backend(&base());
        assert_eq!(p.backend, Backend::S2);
        assert_eq!(p.reason, "lane_s2");
        assert!(!p.downgraded);
    }

    #[test]
    fn absent_s2_downgrades_to_s1() {
        let i = RouteInputs {
            s2_available: false,
            ..base()
        };
        let p = plan_backend(&i);
        assert_eq!(p.backend, Backend::S1);
        assert_eq!(p.reason, "s2_absent");
        assert!(p.downgraded);
    }

    #[test]
    fn absent_s2_and_absent_s1_is_cpu_echo() {
        let i = RouteInputs {
            s2_available: false,
            s1_available: false,
            ..base()
        };
        assert_eq!(plan_backend(&i).backend, Backend::CpuEcho);
    }

    #[test]
    fn power_save_keeps_vram_free() {
        let i = RouteInputs {
            power_save: true,
            ..base()
        };
        let p = plan_backend(&i);
        assert_eq!(p.backend, Backend::S1);
        assert_eq!(p.reason, "power_save");
        assert!(p.downgraded);
    }

    #[test]
    fn oversize_prompt_does_not_go_to_s2() {
        let i = RouteInputs {
            prompt_chars: MAX_S2_CHARS + 1,
            ..base()
        };
        assert_eq!(plan_backend(&i).reason, "prompt_too_long");
        let ok = RouteInputs {
            prompt_chars: MAX_S2_CHARS,
            ..base()
        };
        assert_eq!(plan_backend(&ok).backend, Backend::S2);
    }

    #[test]
    fn interactive_does_not_queue_behind_s2() {
        let i = RouteInputs {
            s2_pending: 1,
            ..base()
        };
        let p = plan_backend(&i);
        assert_eq!(p.backend, Backend::S1);
        assert_eq!(p.reason, "s2_busy_interactive");
        assert!(p.downgraded);
    }

    #[test]
    fn power_save_mapping() {
        assert!(power_save_from_presence("sleep"));
        assert!(power_save_from_presence("DROWSY"));
        assert!(!power_save_from_presence("active"));
        assert!(!power_save_from_presence(""));
    }

    #[test]
    fn background_job_may_queue_on_s2() {
        let i = RouteInputs {
            interactive: false,
            s2_pending: 3,
            ..base()
        };
        assert_eq!(plan_backend(&i).backend, Backend::S2);
    }
}
