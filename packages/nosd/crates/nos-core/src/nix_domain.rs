//! Domínio Nix — factos vivos da máquina + lições caras (AGENTS.md), para que
//! o córtex/hermes respondam com o que ESTA máquina é, não com o que o modelo
//! imagina que um NixOS é.
//!
//! Doutrina (honesty ADR-0113): facto ausente é `None`, nunca um default
//! inventado. `on_nixos=false` é uma resposta legítima (host Windows/lab).

use serde::Serialize;
use std::path::Path;

/// Factos lidos a cada pedido — baratos (4 stats + 1 readlink + glob de perfis).
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct NixFacts {
    /// Existe `/etc/NIXOS` → estamos num NixOS instalado, não num host estranho.
    pub on_nixos: bool,
    pub version: Option<String>,
    /// Closure activa (`/run/current-system`).
    pub system: Option<String>,
    /// Escolha do sistema: `/etc/nixos` (flakes) ou `/etc/nixos/configuration.nix`.
    pub config_dir: Option<String>,
    pub flake: Option<String>,
    /// Nº de gerações do perfil `system` legíveis em `/nix/var/nix/profiles`.
    pub generations: Option<usize>,
    /// `NOSD_NIX_RING=1` no processo — o ring root só é armável com S-04.
    pub ring_armed: bool,
    pub systemd: bool,
    /// `YYMM` do nixpkgs instalado (ex.: `2611`), derivado de `/etc/NIXOS`.
    /// `None` = não provado — uma lição presa a versão não se aplica.
    pub nixpkgs: Option<u32>,
    /// `/dev/nvidia0` ou `/proc/driver/nvidia/version` presentes.
    pub has_nvidia: bool,
    /// `/proc/driver/nvidia/gpus/*/power_readings` ou nvidia-smi disponível → CUDA real.
    pub has_cuda: bool,
    /// Contador RAPL legível em `/sys/class/powercap/*/energy_uj`.
    pub has_rapl: bool,
}

impl NixFacts {
    /// Linha compacta para prompt/HUD (uma linha, sem PII).
    pub fn line(&self) -> String {
        if !self.on_nixos {
            return format!(
                "não-NixOS (host={}); systemd={}",
                std::env::consts::OS,
                yes_no(self.systemd)
            );
        }
        let mut s = String::from("NixOS");
        if let Some(v) = &self.version {
            s.push(' ');
            s.push_str(v);
        }
        if let Some(g) = self.generations {
            s.push_str(&format!(" | gen {g}"));
        }
        if let Some(sy) = &self.system {
            s.push_str(&format!(" | {sy}"));
        }
        s.push_str(&format!(
            " | flake={} | ring={} | systemd={}",
            self.flake.as_deref().unwrap_or("ausente"),
            if self.ring_armed { "ARMED" } else { "disarmed" },
            yes_no(self.systemd)
        ));
        s
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn read_trim(path: &str, max: usize) -> Option<String> {
    let s = std::fs::read_to_string(path).ok()?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.chars().take(max).collect())
    }
}

fn readlink(path: &str) -> Option<String> {
    let p = std::fs::read_link(path).ok()?;
    let mut s = p.to_string_lossy().to_string();
    // `/nix/store/<hash>-nixos-system-host-26.11...` → `<hash>-nixos-system-...`
    if let Some(i) = s.find("/nix/store/") {
        s = s[i + "/nix/store/".len()..].to_string();
    }
    Some(s.chars().take(64).collect())
}

/// Conta perfis `system-NN-link` sem seguir symlinks.
fn count_generations(root: &str) -> Option<usize> {
    let rd = std::fs::read_dir(root).ok()?;
    let mut n = 0usize;
    for e in rd.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if let Some(rest) = name.strip_prefix("system-") {
            if let Some(num) = rest.strip_suffix("-link") {
                if num.chars().all(|c| c.is_ascii_digit()) {
                    n += 1;
                }
            }
        }
    }
    if n == 0 {
        None
    } else {
        Some(n)
    }
}

/// Lê o estado Nix da máquina. Nunca falha: ausência vira `None`.
pub fn probe() -> NixFacts {
    let on_nixos = Path::new("/etc/NIXOS").exists();
    let flake = {
        let f = Path::new("/etc/nixos/flake.nix");
        if f.is_file() {
            Some("/etc/nixos/flake.nix".to_string())
        } else {
            None
        }
    };
    let config_dir = if Path::new("/etc/nixos/configuration.nix").is_file() {
        Some("/etc/nixos/configuration.nix".to_string())
    } else if flake.is_some() {
        Some("/etc/nixos".to_string())
    } else {
        None
    };
    let version = read_trim("/etc/NIXOS", 48);
    NixFacts {
        on_nixos,
        nixpkgs: parse_nixpkgs(version.as_deref()),
        version,
        system: readlink("/run/current-system"),
        config_dir,
        flake,
        generations: count_generations("/nix/var/nix/profiles"),
        ring_armed: std::env::var("NOSD_NIX_RING")
            .map(|v| v == "1")
            .unwrap_or(false),
        systemd: Path::new("/run/systemd/system").is_dir(),
        has_nvidia: Path::new("/dev/nvidia0").exists()
            || Path::new("/proc/driver/nvidia/version").is_file(),
        has_cuda: Path::new("/proc/driver/nvidia/gpus")
            .read_dir()
            .is_ok_and(|rd| rd.flatten().next().is_some()),
        has_rapl: Path::new("/sys/class/powercap")
            .read_dir()
            .is_ok_and(|rd| rd.flatten().next().is_some()),
    }
}

/// `26.11.20240915.abc` → `2611`. Aceita `YY.MM` no início; mais nada.
pub fn parse_nixpkgs(release: Option<&str>) -> Option<u32> {
    let s = release?;
    let mut it = s.split('.');
    let yy = it.next()?;
    let mm = it.next()?;
    // `25.05pre` / `26.11.2024...`: só o prefixo numérico conta.
    let digits = |s: &str| -> Option<u32> {
        let d: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        if d.is_empty() {
            None
        } else {
            d.parse().ok()
        }
    };
    let yy: u32 = digits(yy)?;
    let mm: u32 = digits(mm)?;
    if !(20..=40).contains(&yy) || !(1..=12).contains(&mm) {
        return None;
    }
    Some(yy * 100 + mm)
}

/// Lição operacional destilada do `AGENTS.md` — cada uma custou um build ou
/// uma sessão de metal. `triggers` são termos de intenção, não substring solta.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Lesson {
    pub id: &'static str,
    pub topic: &'static str,
    pub triggers: &'static [&'static str],
    pub text: &'static str,
    /// Pré-condições avaliadas contra os factos vivos. Uma lição cuja
    /// pré-condição é falsa **não** entra no prompt nem no `nix.ask`: é assim
    /// que o conhecimento envelhece sem mentir (nixpkgs novo, sem GPU, ring…).
    pub when: &'static [Pre],
}

/// Condição sob a qual uma lição ainda é verdadeira.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pre {
    /// Só num NixOS instalado.
    OnNixos,
    /// `YYMM` mínimo do nixpkgs (ex.: 2611 = 26.11).
    NixpkgsAtLeast(u32),
    /// Há GPU NVIDIA visível no host.
    HasNvidia,
    /// O ring Nix está desarmado (default seguro).
    RingDisarmed,
    /// CUDA está disponível (nvidia-smi / driver carregado).
    HasCuda,
    /// Contador de energia RAPL disponível (medida real, não estimativa).
    HasRapl,
}

impl serde::Serialize for Pre {
    /// Serializa como o rótulo legível (`nixpkgs>=26.11`), não como enum.
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.label())
    }
}

impl Pre {
    pub fn label(self) -> String {
        match self {
            Pre::OnNixos => "on_nixos".into(),
            Pre::NixpkgsAtLeast(v) => format!("nixpkgs>={}", v / 100),
            Pre::HasNvidia => "has_nvidia".into(),
            Pre::RingDisarmed => "ring_disarmed".into(),
            Pre::HasCuda => "has_cuda".into(),
            Pre::HasRapl => "has_rapl".into(),
        }
    }

    fn holds(self, f: &NixFacts) -> bool {
        match self {
            Pre::OnNixos => f.on_nixos,
            Pre::NixpkgsAtLeast(v) => f.nixpkgs.map(|x| x >= v).unwrap_or(false),
            Pre::HasNvidia => f.has_nvidia,
            Pre::RingDisarmed => !f.ring_armed,
            Pre::HasCuda => f.has_cuda,
            Pre::HasRapl => f.has_rapl,
        }
    }
}

/// A lição aplica-se a estes factos? `Err(motivo)` é o motivo da recusa —
/// usado pelo `audit` e nunca escondido.
pub fn applicability(l: &Lesson, f: &NixFacts) -> Result<(), String> {
    for p in l.when {
        if !p.holds(f) {
            return Err(format!("{} falsa", p.label()));
        }
    }
    Ok(())
}

macro_rules! lesson {
    ($id:literal, $topic:literal, [$($t:literal),*], $text:literal) => {
        lesson!($id, $topic, [$($t),*], $text, [])
    };
    ($id:literal, $topic:literal, [$($t:literal),*], $text:literal, [$($p:expr),*]) => {
        Lesson {
            id: $id,
            topic: $topic,
            triggers: &[$($t),*],
            text: $text,
            when: &[$($p),*],
        }
    };
}

/// Base de conhecimento: **só** o que foi aprendido à força neste projecto.
pub const LESSONS: &[Lesson] = &[
    lesson!(
        "flake-git-tree",
        "build",
        ["nix build", "flake", "ficheiro novo", "untracked", "git add"],
        "O flake usa a árvore git: `git add` de ficheiros novos ANTES de `nix build`, senão o ficheiro não existe na eval."
    ),
    lesson!(
        "unit-attrs",
        "systemd",
        ["startlimit", "unit", "serviceconfig", "path", "serviço", "service"],
        "`path` e `StartLimit*` são atributos do UNIT, nunca de `serviceConfig` — o systemd ignora-os em silêncio."
    ),
    lesson!(
        "shebang-path",
        "systemd",
        ["shebang", "script", "bash", "writeShellScript"],
        "Script com shebang `#!/usr/bin/env bash` precisa de `bash` no `path` do unit, ou morre 127 sem diagnóstico."
    ),
    lesson!(
        "no-self-systemctl",
        "systemd",
        ["systemctl", "polkit", "restart", "unit"],
        "Serviço a correr como `nosd` não pode `systemctl start` (polkit): usar `.path`/ficheiro gatilho e nunca reiniciar o daemon que serve o pedido."
    ),
    lesson!(
        "run-user-tmpfs",
        "runtime",
        ["xdg_runtime_dir", "/run/user", "tmpfs", "logind"],
        "`/run/user` é tmpfs do logind e esconde o que lá foi criado: usar `/run/<servico>/user`."
    ),
    lesson!(
        "watchdog",
        "runtime",
        ["watchdog", "restart", "hang", "pendur", "watchdogsec"],
        "`Restart=` só reage a saída; daemon pendurado precisa de `WatchdogSec=` + `sd_notify(WATCHDOG=1)`. Matar o processo prova o `Restart`, não o watchdog."
    ),
    lesson!(
        "device-allow",
        "hardware",
        ["deviceallow", "nvidia", "cuda", "gpu", "gpu permit"],
        "`DeviceAllow` resolve aliases por /proc/devices: char-nvidia cobre /dev/nvidiaN e /dev/nvidiactl. Aliases inexistentes sao ignorados em silencio e a CUDA morre com ENODEV.",
        [Pre::HasNvidia]
    ),
    lesson!(
        "credential",
        "segurança",
        ["api key", "segredo", "secret", "token", "loadcredential"],
        "Segredos entram por LoadCredential= e $CREDENTIALS_DIRECTORY: nunca em Environment= (visível em systemctl show) nem em argv."
    ),
    lesson!(
        "crlf",
        "build",
        ["crlf", "\\r", "editar", "windows", "fixupphase"],
        "Python `write_text` no Windows grava CRLF e o stdenv morre na fixupPhase com `$'\\r': command not found`. Converter para LF antes do build."
    ),
    lesson!(
        "tr-delete-r",
        "build",
        ["tr -d", "conversão", "crlf strip"],
        "`tr -d '\\r'` em massa apaga a letra `r` (`return`→`retun`). Use `perl -pi -e 's/\\r\\n/\\n/g'`."
    ),
    lesson!(
        "bom",
        "build",
        ["bom", "utf-8", "eval"],
        "BOM UTF-8 no `flake.nix` faz o eval falhar à cabeça; strip do BOM antes do build."
    ),
    lesson!(
        "interp-escape",
        "nix-language",
        ["${", "interpolação", "writeShellScript", "escape"],
        "Dentro de `''…''`, `${VAR}` é interpolação Nix: usar `''${VAR}`. Validar com `nix-instantiate --parse` em cada módulo."
    ),
    lesson!(
        "mkforce-conflict",
        "nix-language",
        ["mkforce", "conflito", "sshd", "override"],
        "`mkForce` contra `mkForce` = conflito de prioridades. Para reabrir um serviço já forçado a false: `lib.mkOverride 10 true`."
    ),
    lesson!(
        "cargo-build-flags",
        "build",
        ["cargoextraargs", "cargo", "26.11", "buildrustpackage"],
        "`cargoExtraArgs` foi removido no nixpkgs 26.11 em silêncio: usar `cargoBuildFlags`.",
        [Pre::NixpkgsAtLeast(2611)]
    ),
    lesson!(
        "firstboot-tty1",
        "boot",
        ["firstboot", "boot", "serial", "tty", "getty"],
        "Firstboot interactivo no metal corre no `tty1`: o getty serial ganha o flock e as perguntas ficam invisíveis. `console=tty0` mantém o boot visível.",
        [Pre::OnNixos]
    ),
    lesson!(
        "cage-vt",
        "gui",
        ["cage", "wayland", "vt", "page-flip", "kiosk"],
        "cage/wlroots só faz page-flip na VT activa (`chvt` no ExecStartPre) e `WAYLAND_DISPLAY` só no filho — no cage = modo aninhado."
    ),
    lesson!(
        "iso-firewall",
        "rede",
        ["firewall", "udp", "porta", "mesh", "demo"],
        "A ISO live tem o firewall NixOS ligado: UDP 19000/TCP 19001 morrem no INPUT. Uma demo tem de o abrir/desligar explicitamente.",
        [Pre::OnNixos]
    ),
    lesson!(
        "qemu-not-metal",
        "honesty",
        ["metal", "qemu", "verified", "aceite", "prova"],
        "QEMU ≠ metal e UI completed ≠ aceite: `VERIFIED` exige artefacto + repetição. `EngineAbsent` é honestidade, não incapacidade."
    ),
    lesson!(
        "ring-disarmed",
        "segurança",
        ["ring", "root", "nix.propose", "apply", "armado"],
        "Ring Nix desarmado (`armRing=false`) é o default correcto: sem `NOSD_NIX_RING=1` o helper root recusa. Auto-melhoria = `nix.propose` → HITL → `nix.confirm`.",
        [Pre::RingDisarmed]
    ),
    lesson!(
        "store-hash",
        "modelos",
        ["modelo", "gguf", "hash", "catálogo", "pin"],
        "Modelos pinam-se por rev + sha256 e verificam-se no fetch; catálogo por store hash evita re-descarregar o mesmo blob."
    ),
];

/// Lições cujo trigger aparece na pergunta (case-insensitive), até `max`.
///
/// Só entram lições **aplicáveis** aos factos desta máquina: uma lição presa a
/// 26.11 não fala num 27.05, e `device-allow` não fala sem GPU NVIDIA.
pub fn lookup(question: &str, max: usize) -> Vec<&'static Lesson> {
    lookup_with(question, max, &probe())
}

/// `lookup` com factos explícitos (testes e `nix.ask` com snapshot).
pub fn lookup_with(question: &str, max: usize, facts: &NixFacts) -> Vec<&'static Lesson> {
    let q = question.to_lowercase();
    let mut scored: Vec<(usize, &'static Lesson)> = Vec::new();
    for l in LESSONS {
        if applicability(l, facts).is_err() {
            continue;
        }
        let score = l
            .triggers
            .iter()
            .filter(|t| q.contains(&t.to_lowercase()))
            .count();
        if score > 0 {
            scored.push((score, l));
        }
    }
    // Score desc, depois id para determinismo (mesma pergunta = mesmo prompt).
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(b.1.id)));
    scored
        .into_iter()
        .take(max.max(1))
        .map(|(_, l)| l)
        .collect()
}

/// Bloco de prompt `## Nix (live)`. `None` quando a pergunta não toca no Nix —
/// não se gasta contexto de um 0.8B com doutrina irrelevante.
pub fn prompt_block(question: &str, max_chars: usize) -> Option<String> {
    let hits = lookup(question, 3);
    let facts = probe();
    if hits.is_empty() && !facts.on_nixos {
        return None;
    }
    let mut s = String::from("## Nix (live)\n");
    s.push_str(&format!("facts: {}\n", facts.line()));
    if facts.on_nixos {
        if let Some(c) = &facts.config_dir {
            s.push_str(&format!("config: {c}\n"));
        }
    }
    if !hits.is_empty() {
        s.push_str("lessons (custaram builds/sessões; aplicar antes de propor mudanças):\n");
        for l in hits {
            let t: String = l.text.chars().take(220).collect();
            s.push_str(&format!("- [{}] {t}\n", l.id));
        }
    }
    if s.len() > max_chars {
        Some(s.chars().take(max_chars).collect())
    } else {
        Some(s)
    }
}

/// Dívida de conhecimento: que lições já não se aplicam a esta máquina e
/// porquê. É o que impede o AGENTS.md de virar folclore depois de um bump.
pub fn audit() -> serde_json::Value {
    let f = probe();
    let mut applies: Vec<&'static str> = Vec::new();
    let mut stale: Vec<serde_json::Value> = Vec::new();
    for l in LESSONS {
        match applicability(l, &f) {
            Ok(()) => applies.push(l.id),
            Err(why) => stale.push(serde_json::json!({ "id": l.id, "why": why })),
        }
    }
    serde_json::json!({
        "facts": f,
        "total": LESSONS.len(),
        "applies": applies.len(),
        "stale": stale.len(),
        "stale_lessons": stale,
        "basis": if f.on_nixos { "observed" } else { "host_nao_nixos" },
    })
}

/// Resumo para HUD/UI (sem doutrina): factos + contagem de lições.
pub fn symbiosis_summary() -> serde_json::Value {
    let f = probe();
    let mut open: Vec<serde_json::Value> = Vec::new();
    for l in LESSONS {
        if let Err(why) = applicability(l, &f) {
            open.push(serde_json::json!({
                "id": l.id,
                "topic": l.topic,
                "text": l.text,
                "why": why,
                // O que a máquina precisaria para a lição voltar a aplicar.
                "hint": hint_for(l, &f),
            }));
        }
    }
    serde_json::json!({
        "facts": f,
        "line": f.line(),
        "lessons": LESSONS.len(),
        "open_lessons": open,
        "open_count": open.len(),
        "topics": {
            "build": LESSONS.iter().filter(|l| l.topic == "build").count(),
            "systemd": LESSONS.iter().filter(|l| l.topic == "systemd").count(),
            "hardware": LESSONS.iter().filter(|l| l.topic == "hardware").count(),
        },
        "honesty": "factos lidos do sistema; ausente = null, nunca estimado",
        "applicable": LESSONS
            .iter()
            .filter(|l| applicability(l, &f).is_ok())
            .count(),
    })
}

/// Hint acionável para fechar uma lição em dívida: deriva da pré-condição
/// que falhou contra os factos vivos (mesma semântica de `Pre::holds`).
fn hint_for(l: &Lesson, f: &NixFacts) -> &'static str {
    for p in l.when {
        match p {
            Pre::OnNixos if !f.on_nixos => {
                return "Host não é NixOS — esta lição só vale dentro da ISO/VM NIXAIOS"
            }
            Pre::NixpkgsAtLeast(v) if !f.nixpkgs.map(|x| x >= *v).unwrap_or(false) => {
                return "nixpkgs abaixo do mínimo da lição — novo build/instalação resolve"
            }
            Pre::HasNvidia if !f.has_nvidia => {
                return "GPU NVIDIA não visível — a lição re-aplica quando a dGPU aparece"
            }
            Pre::RingDisarmed if f.ring_armed => {
                return "Ring armado fora do default seguro — rever S-04/ADR antes de confiar"
            }
            Pre::HasCuda if !f.has_cuda => {
                return "CUDA não detectado — a lição re-aplica quando o driver NVIDIA carregar"
            }
            Pre::HasRapl if !f.has_rapl => {
                return "RAPL ausente — energia fica em tdp_estimate (limite inferior), nunca 0"
            }
            _ => {}
        }
    }
    "Pré-condição não cumprida nesta máquina — ver `why`"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_never_panics_and_is_honest() {
        let f = probe();
        if !f.on_nixos {
            assert!(f.version.is_none(), "fora do NixOS não há versão");
        }
        // linha sempre produzível
        assert!(!f.line().is_empty());
    }

    #[test]
    fn symbiosis_exposes_open_lessons_with_hint() {
        let s = symbiosis_summary();
        let open = s["open_lessons"].as_array().unwrap();
        assert_eq!(open.len(), s["open_count"].as_u64().unwrap() as usize);
        for l in open {
            assert!(l.get("id").is_some(), "lição aberta sem id");
            assert!(l.get("hint").is_some(), "lição aberta sem hint acionável");
        }
    }

    #[test]
    fn lookup_matches_triggers_not_noise() {
        let l = lookup("o serviço systemd não arranca e não sei porquê", 2);
        assert!(!l.is_empty());
        assert!(l.iter().any(|x| x.topic == "systemd"));
        assert!(lookup("qual é a capital de França", 3).is_empty());
    }

    #[test]
    fn lookup_is_deterministic() {
        let a: Vec<_> = lookup("nix build falha no flake", 3)
            .iter()
            .map(|l| l.id)
            .collect();
        let b: Vec<_> = lookup("nix build falha no flake", 3)
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn prompt_block_omits_unrelated_questions() {
        // Host Windows/lab, pergunta sem Nix → nada injectado.
        if !probe().on_nixos {
            assert!(prompt_block("receita de bolo", 900).is_none());
        }
    }

    #[test]
    fn prompt_block_is_capped() {
        if let Some(b) = prompt_block("nix build flake systemd cuda", 120) {
            assert!(b.chars().count() <= 120, "bloco tem de respeitar o tecto");
        }
    }

    #[test]
    fn nixpkgs_is_parsed_or_none() {
        assert_eq!(parse_nixpkgs(Some("26.11.20240915.abc")), Some(2611));
        assert_eq!(parse_nixpkgs(Some("25.05pre")), Some(2505));
        assert_eq!(parse_nixpkgs(Some("lixo")), None);
        assert_eq!(parse_nixpkgs(Some("26")), None);
        assert_eq!(parse_nixpkgs(None), None);
    }

    #[test]
    fn preconditions_drop_lessons_that_no_longer_hold() {
        let f = NixFacts::default(); // fora do NixOS, ring desarmado, sem nvidia
        let ids: Vec<&str> = lookup_with("ring root nix.propose apply", 6, &f)
            .iter()
            .map(|l| l.id)
            .collect();
        assert!(ids.contains(&"ring-disarmed"), "{ids:?}");

        let armed = NixFacts {
            ring_armed: true,
            ..NixFacts::default()
        };
        let ids2: Vec<&str> = lookup_with("ring root nix.propose apply", 6, &armed)
            .iter()
            .map(|l| l.id)
            .collect();
        assert!(!ids2.contains(&"ring-disarmed"), "ring armado: {ids2:?}");
    }

    #[test]
    fn version_bound_lesson_needs_the_version() {
        let old = NixFacts {
            nixpkgs: Some(2505),
            ..NixFacts::default()
        };
        let new = NixFacts {
            nixpkgs: Some(2611),
            ..NixFacts::default()
        };
        let has = |facts: &NixFacts, id: &str| {
            lookup_with("cargoextraargs buildrustpackage", 5, facts)
                .iter()
                .any(|l| l.id == id)
        };
        assert!(!has(&old, "cargo-build-flags"));
        assert!(has(&new, "cargo-build-flags"));
    }

    #[test]
    fn gpu_lesson_needs_a_gpu() {
        let no_gpu = NixFacts::default();
        let gpu = NixFacts {
            has_nvidia: true,
            ..NixFacts::default()
        };
        let has = |facts: &NixFacts| {
            lookup_with("cuda deviceallow nvidia", 5, facts)
                .iter()
                .any(|l| l.id == "device-allow")
        };
        assert!(!has(&no_gpu));
        assert!(has(&gpu));
    }

    #[test]
    fn audit_counts_never_exceed_total() {
        let a = audit();
        let total = a["total"].as_u64().unwrap();
        let applies = a["applies"].as_u64().unwrap();
        let stale = a["stale"].as_u64().unwrap();
        assert_eq!(total, applies + stale);
        assert_eq!(total as usize, LESSONS.len());
    }

    #[test]
    fn lessons_are_unique_and_short() {
        let mut ids = std::collections::BTreeSet::new();
        for l in LESSONS {
            assert!(ids.insert(l.id), "id duplicado: {}", l.id);
            assert!(l.text.len() <= 260, "lição longa demais: {}", l.id);
            assert!(!l.triggers.is_empty(), "lição sem gatilho: {}", l.id);
        }
    }
}
