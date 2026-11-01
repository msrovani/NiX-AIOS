//! Brain self-upgrade (ADR-0118 addenda `0118-qwen-embedded-brain.md`).
//!
//! Observe→Plan→Act com HITL server-authored:
//! - `plan()`: posture OBSERVADO → tier alcançável mais alto (quant melhor no
//!   mesmo modelo; modelo maior; ou dGPU), com custo **exato** do candidato
//!   (bytes/MB pinados) e ganho descrito honesto (sem tokens/s inventados).
//! - `accept()`: só depois de grant HITL (`confirm_id`, kind `brain.upgrade`).
//!   Fetch + **verificação do sha256 pinado** + hot_load + registo no hub.
//!
//! Honesty: o fetch é `fetch_to_dir` runtime (até G4.1 fechar o ciclo
//! modelo-como-derivação). O pin sha256 é o LFS oid do HF tree — download com
//! hash diferente é recusado E apagado (fail-closed).
//! Fonte de confiança dos candidatos: unsloth (Apache-2.0), base_model
//! Qwen/Qwen3.5-*(base tier 1 = brain embutido na iso-hw, Q4_K_M).

use nos_cortex::NodeCapacity;
use serde::Serialize;

/// Candidato pinado do registo de tiers.
pub struct TierCandidate {
    pub tier: u8,
    pub name: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub size_bytes: u64,
    pub sha256: &'static str,
    pub params_label: &'static str,
    pub scale_vs_08b: &'static str,
    /// RAM avail mínima para INFERÊNCIA CPU/iGPU (pesos + runtime/KV).
    pub min_ram_mb: u64,
    /// VRAM mínima para o tier dGPU (0 = CPU/iGPU só).
    pub min_vram_mb: u64,
}

/// Tier 2: mesma rede do brain embutido, quantização Q4_K_M → Q8_0.
pub static TIER_Q8: TierCandidate = TierCandidate {
    tier: 2,
    name: "Qwen3.5-0.8B Q8_0",
    repo: "unsloth/Qwen3.5-0.8B-GGUF",
    file: "Qwen3.5-0.8B-Q8_0.gguf",
    size_bytes: 811_843_840,
    sha256: "0ad885ffd4bb022fc4f0d33a3308fa108ef8613159d3b3a67e23abca056b7a6c",
    params_label: "0.8B",
    scale_vs_08b: "1x parâmetros; precisão 4-bit → 8-bit",
    min_ram_mb: 1_700,
    min_vram_mb: 0,
};

/// Tier 3: modelo 5x maior em Q4_K_M, CPU/iGPU.
pub static TIER_4B: TierCandidate = TierCandidate {
    tier: 3,
    name: "Qwen3.5-4B Q4_K_M",
    repo: "unsloth/Qwen3.5-4B-GGUF",
    file: "Qwen3.5-4B-Q4_K_M.gguf",
    size_bytes: 2_740_937_888,
    sha256: "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4",
    params_label: "4B",
    scale_vs_08b: "5x parâmetros vs 0.8B",
    min_ram_mb: 4_300,
    min_vram_mb: 0,
};

/// Tier 3b/6 GB: mesma família 4B, Q6_K (3,29 GiB) — mais precisão que Q4
/// sem chegar aos 5,68 GB do 9B. Contexto máximo fica com o `--fit` do
/// llama-server (sem `-c` fixo).
pub static TIER_4B_Q6: TierCandidate = TierCandidate {
    tier: 3,
    name: "Qwen3.5-4B Q6_K",
    repo: "unsloth/Qwen3.5-4B-GGUF",
    file: "Qwen3.5-4B-Q6_K.gguf",
    size_bytes: 3_525_956_768,
    sha256: "fdedd781c9ce676ab66b018ca247ff78e8a33c98098a822c1e2d5075e7718f66",
    params_label: "4B",
    scale_vs_08b: "5x parâmetros vs 0.8B; Q6_K mais preciso que Q4_K_M",
    min_ram_mb: 4_800,
    min_vram_mb: 0,
};

/// Tier 4: modelo 11x maior; alvo REAL é dGPU supervisionada (nosdEngine).
/// Em CPU honesto exige RAM confortável (o head de 7,7 GB est. já OOM ≤8 GiB).
pub static TIER_9B: TierCandidate = TierCandidate {
    tier: 4,
    name: "Qwen3.5-9B Q4_K_M",
    repo: "unsloth/Qwen3.5-9B-GGUF",
    file: "Qwen3.5-9B-Q4_K_M.gguf",
    size_bytes: 5_680_522_464,
    sha256: "03b74727a860a56338e042c4420bb3f04b2fec5734175f4cb9fa853daf52b7e8",
    params_label: "9B",
    scale_vs_08b: "11x parâmetros vs 0.8B",
    min_ram_mb: 7_700,
    min_vram_mb: 6_600,
};

pub static ALL_TIERS: [&TierCandidate; 4] = [&TIER_Q8, &TIER_4B_Q6, &TIER_4B, &TIER_9B];

fn hf_url(c: &TierCandidate) -> String {
    format!("https://huggingface.co/{}/resolve/main/{}", c.repo, c.file)
}

fn mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

/// Escolhe o tier ALCANÇÁVEL mais alto dado o posture OBSERVADO.
/// - tier 4 (9B): VRAM ≥ pino (hot_load local; ring só para nix root) **ou**
///   ring ARMADO + RAM CPU. Frota: 4070 12GB sobe; 3050 não.
/// - tiers 2/3: CPU/iGPU, RAM avail manda.
pub fn pick_reachable_tier(cap: &NodeCapacity, ring_armed: bool) -> Option<&'static TierCandidate> {
    let tier4_vram = cap.vram_free_mb >= TIER_9B.min_vram_mb;
    let tier4_cpu = ring_armed && cap.ram_avail_mb >= TIER_9B.min_ram_mb + 2_000;
    if tier4_vram || tier4_cpu {
        return Some(&TIER_9B);
    }
    // 6 GB / mesma família: Q6_K antes do Q4 quando há RAM para o runtime.
    if cap.ram_avail_mb >= TIER_4B_Q6.min_ram_mb {
        return Some(&TIER_4B_Q6);
    }
    if cap.ram_avail_mb >= TIER_4B.min_ram_mb {
        return Some(&TIER_4B);
    }
    if cap.ram_avail_mb >= TIER_Q8.min_ram_mb {
        return Some(&TIER_Q8);
    }
    None
}

/// Direcção da proposta HITL (upgrade / downgrade / hint de driver).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptDirection {
    Upgrade,
    Downgrade,
    DriverHint,
    Stay,
}

/// Fit bidirecional: se posture degradado (sem VRAM / CPU apertado) propõe
/// **menor**; se VRAM folga (ex. 4070 12GB) propõe **maior**. HITL decide.
pub fn pick_adapt_tier(
    cap: &NodeCapacity,
    current_bytes: Option<u64>,
    ring_armed: bool,
    cuda_degraded: bool,
) -> (AdaptDirection, Option<&'static TierCandidate>) {
    let up = pick_reachable_tier(cap, ring_armed);
    // Degradado: GPU ignored / CUDA fail → preferir 0.8B Q4 embutido ou Q8 se RAM ok,
    // nunca empurrar 9B; se current já é 4B+ e CPU, propor 4B Q4 ou 0.8B.
    if cuda_degraded || (cap.vram_free_mb == 0 && current_bytes.unwrap_or(0) > TIER_4B.size_bytes) {
        let down = if cap.ram_avail_mb < TIER_4B.min_ram_mb {
            Some(&TIER_Q8)
        } else if cap.ram_avail_mb < TIER_4B_Q6.min_ram_mb {
            Some(&TIER_4B)
        } else {
            // Ainda cabe 4B; propor Q4 mais leve que Q6 se current parece Q6+.
            if current_bytes.unwrap_or(0) >= TIER_4B_Q6.size_bytes {
                Some(&TIER_4B)
            } else {
                None
            }
        };
        if let Some(t) = down {
            if current_bytes.map(|b| b > t.size_bytes).unwrap_or(true) {
                return (AdaptDirection::Downgrade, Some(t));
            }
        }
        if cuda_degraded && cap.vram_free_mb == 0 {
            // Hint: Pascal precisa legacy 580 — card informativa sem fetch.
            return (AdaptDirection::DriverHint, None);
        }
    }
    match up {
        Some(t) => {
            let bigger = current_bytes.map(|b| t.size_bytes > b).unwrap_or(true);
            if bigger {
                (AdaptDirection::Upgrade, Some(t))
            } else {
                (AdaptDirection::Stay, None)
            }
        }
        None => (AdaptDirection::Stay, None),
    }
}

#[derive(Debug, Serialize)]
pub struct BrainUpgradePlan {
    /// Posture observado (nunca fake).
    pub observed: Observed,
    /// Brain em execução (label honesto; pode ser "boot default").
    pub current: CurrentBrain,
    /// `None` = stay (sem headroom honesto; brain actual é o correto).
    pub proposal: Option<Proposal>,
    /// Rollback honesto (o anterior continua em models/store).
    pub rollback: String,
    pub honesty: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Observed {
    pub ram_avail_mb: u64,
    pub vram_free_mb: u64,
    pub posture: String,
    pub ring_armed: bool,
}

#[derive(Debug, Serialize)]
pub struct CurrentBrain {
    pub label: String,
    pub engine: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct Proposal {
    pub tier: u8,
    pub direction: AdaptDirection,
    pub candidate: TierView,
    pub run_target: RunTarget,
    /// Custo exato do pin + estimativas rotuladas como "est.".
    pub cost: Cost,
    pub gain: Gain,
    pub requires: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TierView {
    pub name: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub size_bytes: u64,
    pub sha256: &'static str,
    pub params_label: &'static str,
    pub scale_vs_08b: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RunTarget {
    pub engine: String,
    pub device: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Cost {
    pub download_mb: u64,
    pub disk_mb: u64,
    /// "est." — pesos + runtime/KV (nunca apresentado como exato).
    pub ram_est_mb: u64,
    /// Tier dGPU; 0 = não usa NVRAM.
    pub vram_est_mb: u64,
    pub time: &'static str,
}

impl Cost {
    pub fn estimate(tier: &TierCandidate, vram_cuda: bool) -> Cost {
        let weights_mb = mb(tier.size_bytes);
        let ram_est = (weights_mb + 400) * 115 / 100;
        let vram_est = (weights_mb + 900) * 110 / 100;
        Cost {
            download_mb: weights_mb,
            disk_mb: weights_mb,
            ram_est_mb: if vram_cuda { ram_est / 4 } else { ram_est },
            vram_est_mb: if vram_cuda { vram_est } else { 0 },
            time: "depends on net (bytes são pinados: os MB acima são exatos)",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Gain {
    pub size_label: &'static str,
    pub precision: &'static str,
    pub honesty_note: &'static str,
}

impl Gain {
    pub fn describe(tier: &TierCandidate) -> Gain {
        Gain {
            size_label: tier.scale_vs_08b,
            precision: if tier.tier == 2 {
                "pesos idênticos; menos erro de quantização (Q4→Q8)"
            } else {
                "Q4_K_M; mais parâmetros ⇒ mais qualidade geral"
            },
            honesty_note: "ganho qualitativo; p95 medido DEPOIS do hot_load, não prometido",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BrainUpgradePlanFullWrapper;

impl BrainUpgradePlanFullWrapper {
    // espacio vazio: nomes planos acima são a API pública.
}

/// Plan completo (RPC `brain.plan` / gancho pós-boot).
pub fn plan(
    cap: &NodeCapacity,
    current_label: &str,
    current_bytes: Option<u64>,
    engine: &str,
    ring_armed: bool,
) -> BrainUpgradePlan {
    let observed = Observed {
        ram_avail_mb: cap.ram_avail_mb,
        vram_free_mb: cap.vram_free_mb,
        posture: nos_cortex::CapacityProber::vram_posture(cap).to_string(),
        ring_armed,
    };
    let current = CurrentBrain {
        label: current_label.to_string(),
        engine: engine.to_string(),
        bytes: current_bytes,
    };
    let rollback = "normal: `nosctl load <path anterior>` — o GGUF anterior continua em \
                    /var/lib/nosd/models (ou no store via symlink L+ do nosdBrain); \
                    rollback é local, sem rede"
        .to_string();
    let cuda_degraded = cap.vram_free_mb == 0
        && (cap.vendor_hint == "nvidia"
            || std::path::Path::new("/sys/module/nvidia").exists());
    let (direction, tier) = pick_adapt_tier(cap, current_bytes, ring_armed, cuda_degraded);
    match (direction, tier) {
        (AdaptDirection::DriverHint, _) => BrainUpgradePlan {
            observed,
            current,
            proposal: None,
            rollback,
            honesty: "driver_hint — GPU NVIDIA presente sem VRAM (ex. Pascal+595); \
                      CUDA pede legacy 580 (ISO opcional). HITL kind=brain.driver_hint; \
                      sem fetch automático",
        },
        (_, None) | (AdaptDirection::Stay, _) => BrainUpgradePlan {
            observed,
            current,
            proposal: None,
            rollback,
            honesty: "stay — sem headroom/downgrade honesto; brain actual é o correto",
        },
        (dir, Some(t)) => {
            let vram_cuda = t.min_vram_mb > 0 && cap.vram_free_mb >= t.min_vram_mb;
            let hitl_kind = match dir {
                AdaptDirection::Downgrade => "brain.downgrade",
                AdaptDirection::Upgrade => "brain.upgrade",
                AdaptDirection::DriverHint | AdaptDirection::Stay => "brain.upgrade",
            };
            let run_target = RunTarget {
                engine: if vram_cuda {
                    "llama-server CUDA hot_load local (ring só para nix root)".to_string()
                } else {
                    format!("llama-server managed ({engine})")
                },
                device: if vram_cuda { "dGPU" } else { "CPU/iGPU" },
                note: if matches!(dir, AdaptDirection::Downgrade) {
                    "posture degradado: propor modelo/quant MENOR adequado ao HW — só com HITL"
                } else if vram_cuda {
                    "VRAM folga (ex. 4070 12GB): propor modelo MAIOR — hot_load após HITL"
                } else {
                    "spawn managed: troca é runtime (rollback = load do anterior)"
                },
            };
            BrainUpgradePlan {
                observed,
                current,
                proposal: Some(Proposal {
                    tier: t.tier,
                    direction: dir,
                    candidate: TierView {
                        name: t.name,
                        repo: t.repo,
                        file: t.file,
                        size_bytes: t.size_bytes,
                        sha256: t.sha256,
                        params_label: t.params_label,
                        scale_vs_08b: t.scale_vs_08b,
                    },
                    run_target,
                    cost: Cost::estimate(t, vram_cuda),
                    gain: Gain::describe(t),
                    requires: vec![
                        format!(
                            "ram_avail_mb >= {} (observado {})",
                            t.min_ram_mb, cap.ram_avail_mb
                        ),
                        if t.tier == 4 {
                            format!(
                                "vram_free_mb >= {} (observado {}) — hot_load sem ring",
                                t.min_vram_mb, cap.vram_free_mb
                            )
                        } else {
                            "dGPU não obrigatório".to_string()
                        },
                        format!(
                            "grant HITL (kind={hitl_kind}; server-authored, single-use)"
                        ),
                    ],
                }),
                rollback,
                honesty: "custos p95/rem observados depois; nada automático — só com grant HITL",
            }
        }
    }
}

/// ID determinístico da proposta (memória anti-spam 24h do gancho pós-boot).
pub fn tier_id(t: &TierCandidate) -> String {
    format!("{}|{}|{}", t.tier, t.name, t.size_bytes)
}

#[derive(Debug)]
pub enum AcceptError {
    /// Tier não aplicável sob o posture ACTUAL (honestidade na execução).
    NotApplicable(String),
    Fetch(String),
    Sha(String),
}

impl std::fmt::Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotApplicable(s) => write!(f, "tier não aplicável: {s}"),
            Self::Fetch(s) => write!(f, "fetch falhou: {s}"),
            Self::Sha(s) => write!(f, "sha pin diverge: {s}"),
        }
    }
}

/// sha256 hex de um ficheiro (streaming — honesto para GGUF até 5,29 GiB).
pub fn sha256_file(path: &std::path::Path) -> Result<String, AcceptError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| AcceptError::Sha(e.to_string()))?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| AcceptError::Sha(e.to_string()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    use std::fmt::Write as _;
    let mut out = String::new();
    for b in h.finalize() {
        let _ = write!(out, "{b:02x}");
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct AcceptOutcome {
    pub tier: u8,
    pub name: &'static str,
    pub path: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// Act do upgrade (grant HITL já verificado no dispatch):
/// fetch → sha pin → (o caller faz hot_load + hub register + OPAR).
pub fn accept(
    tier_wanted: u8,
    cap: &NodeCapacity,
    ring_armed: bool,
    models_dir: &std::path::Path,
) -> Result<AcceptOutcome, AcceptError> {
    let reachable = pick_reachable_tier(cap, ring_armed);
    // Dois candidatos partilham o tier 3 (4B Q6 e 4B Q4): aceitar o tier
    // significa aceitar o candidato que o MESMO pick considera alcançável,
    // não o primeiro da lista.
    let t = match reachable {
        Some(c) if c.tier == tier_wanted => c,
        _ => ALL_TIERS
            .iter()
            .copied()
            .find(|t| t.tier == tier_wanted)
            .ok_or_else(|| {
                AcceptError::NotApplicable(format!("tier {tier_wanted} desconhecido"))
            })?,
    };
    // Tier 4: hot_load local com VRAM observada (4070 12GB) sem ring.
    // Ring continua a gatear só nix apply / engine declarativo root.
    let tier4_vram_ok = t.tier == 4 && cap.vram_free_mb >= TIER_9B.min_vram_mb;
    if t.tier == 4 && !ring_armed && !tier4_vram_ok {
        return Err(AcceptError::NotApplicable(
            "tier 4 sem VRAM suficiente nem ring — escolhe tier <= 3 ou arma ring".into(),
        ));
    }
    // O mesmo pick da proposta corre DE NOVO na execução (anti-waste):
    // root com grant brain.upgrade não força um tier sem headroom honesto.
    let reachable_tier = reachable.map(|c| c.tier).unwrap_or(0);
    if tier_wanted > reachable_tier {
        return Err(AcceptError::NotApplicable(format!(
            "tier {tier_wanted} excede o headroom honesto actual (candidato = tier {reachable_tier})"
        )));
    }
    let url = hf_url(t);
    let path = nos_cortex::fetch_to_dir(&url, models_dir, t.file)
        .map_err(|e| AcceptError::Fetch(e.to_string()))?;
    let sha = sha256_file(&path)?;
    if sha != t.sha256 {
        // Download com hash diferente do pin = recusado E apagado.
        let _ = std::fs::remove_file(&path);
        return Err(AcceptError::Sha(format!(
            "pin {} ≠ download {sha}",
            t.sha256
        )));
    }
    Ok(AcceptOutcome {
        tier: t.tier,
        name: t.name,
        path,
        bytes: t.size_bytes,
        sha256: t.sha256,
    })
}

/// Corpo da card HITL (texto puro — HUD Slint/Chromium).
pub fn card_body(p: &BrainUpgradePlan) -> String {
    match &p.proposal {
        None => format!(
            "brain upgrade: none (stay)\nobserved: ram={}MB vram={}MB posture={}\n{}",
            p.observed.ram_avail_mb, p.observed.vram_free_mb, p.observed.posture, p.honesty
        ),
        Some(pr) => format!(
            "PROPOSTA {:?} tier {}\nmodelo: {} ({repo}/{file})\n\
             ganho: {params} · {scale}\n\
             CUSTO: download {dl}MB · disco {dk}MB · RAM est. {ram}MB · VRAM est. {vrm}MB\n\
             alvo: {dev} ({eng})\nHONESTY: {note}\nrollback: {rb}\npin sha256={sha}",
            pr.direction,
            pr.tier,
            pr.candidate.name,
            repo = pr.candidate.repo,
            file = pr.candidate.file,
            params = pr.candidate.params_label,
            scale = pr.candidate.scale_vs_08b,
            dl = pr.cost.download_mb,
            dk = pr.cost.disk_mb,
            ram = pr.cost.ram_est_mb,
            vrm = pr.cost.vram_est_mb,
            dev = pr.run_target.device,
            eng = pr.run_target.engine,
            note = pr.gain.honesty_note,
            rb = p.rollback,
            sha = pr.candidate.sha256,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(ram: u64, vram: u64) -> NodeCapacity {
        NodeCapacity {
            ram_total_mb: ram + 512,
            ram_avail_mb: ram,
            vram_free_mb: vram,
            dri_cards: vec![],
            dri_render: vec![],
            vendor_hint: "none".into(),
            accel: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn tiny_ram_stays() {
        let p = plan(
            &cap(1_024, 0),
            "Qwen3.5-0.8B Q4_K_M",
            Some(532_517_120),
            "llama-server-http",
            false,
        );
        assert!(p.proposal.is_none(), "1 GiB RAM: stay");
        assert!(!p.observed.ring_armed);
    }

    #[test]
    fn mid_ram_offers_q8_not_4b() {
        let p = plan(
            &cap(2_000, 0),
            "boot default",
            None,
            "llama-server-http",
            false,
        );
        let t = p.proposal.expect("2 GiB RAM: proposta Q8");
        assert_eq!(t.tier, 2);
        assert_eq!(t.cost.download_mb, 774);
        assert_eq!(t.run_target.device, "CPU/iGPU");
    }

    #[test]
    fn big_ram_offers_4b() {
        let p = plan(
            &cap(8_000, 0),
            "boot default",
            None,
            "llama-server-http",
            false,
        );
        let t = p.proposal.expect("8 GiB RAM: proposta 4B Q6");
        assert_eq!(t.tier, 3);
        assert_eq!(t.candidate.size_bytes, 3_525_956_768);
        assert_eq!(t.candidate.name, "Qwen3.5-4B Q6_K");
        assert!(t.cost.ram_est_mb > 0);
    }

    #[test]
    fn six_gb_profile_prefers_q6_over_q4() {
        // 6 GB VRAM, 5 GiB RAM, sem ring: Q6_K (mesma família) antes do Q4.
        let p = plan(&cap(5_200, 6_000), "boot default", None, "eng", false);
        let t = p.proposal.expect("6 GB: Q6");
        assert_eq!(t.tier, 3);
        assert_eq!(t.candidate.file, "Qwen3.5-4B-Q6_K.gguf");
        assert_eq!(t.run_target.device, "CPU/iGPU");
    }

    #[test]
    fn low_ram_tier3_still_picks_q4() {
        // 4,4 GiB RAM: Q6 não tem headroom (min 4,8 GiB); o Q4 continua o
        // candidato honesto do tier 3.
        let p = plan(&cap(4_400, 0), "boot default", None, "eng", false);
        let t = p.proposal.expect("4,4 GiB: Q4");
        assert_eq!(t.tier, 3);
        assert_eq!(t.candidate.file, "Qwen3.5-4B-Q4_K_M.gguf");
    }

    #[test]
    fn tier4_vram_hot_load_without_ring() {
        // 4070 12GB: VRAM ≥ pin → tier4 hot_load local sem ring (frota-3).
        let p1 = plan(&cap(4_400, 9_000), "boot default", None, "eng", false);
        let t = p1.proposal.expect("tier4 vram");
        assert_eq!(t.tier, 4);
        assert_eq!(t.direction, AdaptDirection::Upgrade);
        assert_eq!(t.run_target.device, "dGPU");
        assert!(t.cost.vram_est_mb > 0);
        // Sem VRAM nem RAM-alta → stay.
        let p2 = plan(&cap(1_024, 0), "boot default", None, "eng", false);
        assert!(p2.proposal.is_none(), "1 GiB RAM: stay");
    }

    #[test]
    fn accept_refuses_tier_above_headroom() {
        let dir = tempfile::tempdir().unwrap();
        let e = accept(3, &cap(2_000, 0), false, dir.path()).unwrap_err();
        assert!(e.to_string().contains("não aplicável"), "{e}");
    }

    #[test]
    fn accept_refuses_tier4_without_vram_or_ring() {
        let dir = tempfile::tempdir().unwrap();
        let e = accept(4, &cap(16_000, 0), false, dir.path()).unwrap_err();
        assert!(
            e.to_string().contains("ring") || e.to_string().contains("VRAM"),
            "{e}"
        );
    }

    #[test]
    fn adapt_downgrade_when_current_heavy_on_cpu() {
        let (dir, t) = pick_adapt_tier(
            &cap(5_000, 0),
            Some(TIER_4B_Q6.size_bytes),
            false,
            true,
        );
        assert_eq!(dir, AdaptDirection::Downgrade);
        assert_eq!(t.expect("down").file, "Qwen3.5-4B-Q4_K_M.gguf");
    }

    #[test]
    fn card_body_has_costs_and_pin() {
        let p = plan(
            &cap(3_000, 0),
            "boot default",
            None,
            "llama-server-http",
            false,
        );
        let card = card_body(&p);
        assert!(card.contains("CUSTO"));
        assert!(card.contains("download"));
        assert!(card.contains("pin sha256="));
        assert!(card.contains("rollback"));
    }
}
