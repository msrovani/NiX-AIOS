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
    NixFacts {
        on_nixos,
        version: read_trim("/etc/NIXOS", 48),
        system: readlink("/run/current-system"),
        config_dir,
        flake,
        generations: count_generations("/nix/var/nix/profiles"),
        ring_armed: std::env::var("NOSD_NIX_RING")
            .map(|v| v == "1")
            .unwrap_or(false),
        systemd: Path::new("/run/systemd/system").is_dir(),
    }
}

/// Lição operacional destilada do `AGENTS.md` — cada uma custou um build ou
/// uma sessão de metal. `triggers` são termos de intenção, não substring solta.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Lesson {
    pub id: &'static str,
    pub topic: &'static str,
    pub triggers: &'static [&'static str],
    pub text: &'static str,
}

macro_rules! lesson {
    ($id:literal, $topic:literal, [$($t:literal),*], $text:literal) => {
        Lesson {
            id: $id,
            topic: $topic,
            triggers: &[$($t),*],
            text: $text,
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
        "`DeviceAllow` resolve aliases por `/proc/devices`: `char-nvidia` cobre `/dev/nvidiaN` e `/dev/nvidiactl`. Aliases inexistentes são ignorados em silêncio e a CUDA morre com ENODEV."
    ),
    lesson!(
        "credential",
        "segurança",
        ["api key", "segredo", "secret", "token", "loadcredential"],
        "Segredos entram por `LoadCredential=` e `$CREDENTIALS_DIRECTORY`: nunca em `Environment=` (visível em `systemctl show`) nem em argv."
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
        "`cargoExtraArgs` foi removido no nixpkgs 26.11 em silêncio: usar `cargoBuildFlags`."
    ),
    lesson!(
        "firstboot-tty1",
        "boot",
        ["firstboot", "boot", "serial", "tty", "getty"],
        "Firstboot interactivo no metal corre no `tty1`: o getty serial ganha o flock e as perguntas ficam invisíveis. `console=tty0` mantém o boot visível."
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
        "A ISO live tem o firewall NixOS ligado: UDP 19000/TCP 19001 morrem no INPUT. Uma demo tem de o abrir/desligar explicitamente."
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
        "Ring Nix desarmado (`armRing=false`) é o default correcto: sem `NOSD_NIX_RING=1` o helper root recusa. Auto-melhoria = `nix.propose` → HITL → `nix.confirm`."
    ),
    lesson!(
        "store-hash",
        "modelos",
        ["modelo", "gguf", "hash", "catálogo", "pin"],
        "Modelos pinam-se por rev + sha256 e verificam-se no fetch; catálogo por store hash evita re-descarregar o mesmo blob."
    ),
];

/// Lições cujo trigger aparece na pergunta (case-insensitive), até `max`.
pub fn lookup(question: &str, max: usize) -> Vec<&'static Lesson> {
    let q = question.to_lowercase();
    let mut scored: Vec<(usize, &'static Lesson)> = Vec::new();
    for l in LESSONS {
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

/// Resumo para HUD/UI (sem doutrina): factos + contagem de lições.
pub fn symbiosis_summary() -> serde_json::Value {
    let f = probe();
    serde_json::json!({
        "facts": f,
        "line": f.line(),
        "lessons": LESSONS.len(),
        "topics": {
            "build": LESSONS.iter().filter(|l| l.topic == "build").count(),
            "systemd": LESSONS.iter().filter(|l| l.topic == "systemd").count(),
            "hardware": LESSONS.iter().filter(|l| l.topic == "hardware").count(),
        },
        "honesty": "factos lidos do sistema; ausente = null, nunca estimado",
    })
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
    fn lessons_are_unique_and_short() {
        let mut ids = std::collections::BTreeSet::new();
        for l in LESSONS {
            assert!(ids.insert(l.id), "id duplicado: {}", l.id);
            assert!(l.text.len() <= 260, "lição longa demais: {}", l.id);
            assert!(!l.triggers.is_empty(), "lição sem gatilho: {}", l.id);
        }
    }
}
