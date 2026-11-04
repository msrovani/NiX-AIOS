//! Autoconhecimento do nó: quem é, o que **pode**, o que **não pode**, como é
//! feito por dentro e onde procurar. Uma única fonte, para o agente e para o
//! prompt.
//!
//! Regra de honestidade (ADR-0113): o catálogo de capacidades é a **mesma**
//! tabela que o `nos-ipc::Request::required_cap` autoriza — um teste em
//! `nos-ipc` falha se as duas divergirem. Nada aqui é inventado: versão vem
//! do crate, Nix de `nix_domain::probe`, limites de factos observados e de
//! flags do runtime (`SelfEnv`). O que não se sabe é `None`, nunca um default
//! que pareça conhecimento.

use crate::cap::CapBits;
use crate::nix_domain;
use serde::Serialize;

/// Um método IPC e o que ele exige/custa. `side_effect` = muda o mundo ou o
/// estado (escrita, rede, spawn, root), não apenas leitura.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Capability {
    pub method: &'static str,
    pub domain: &'static str,
    pub caps: CapBits,
    pub side_effect: bool,
}

const fn c(
    method: &'static str,
    domain: &'static str,
    caps: CapBits,
    side_effect: bool,
) -> Capability {
    Capability {
        method,
        domain,
        caps,
        side_effect,
    }
}

const NONE: CapBits = CapBits::NONE;
const INFER: CapBits = CapBits::INFER;
const NET: CapBits = CapBits::NET;
const FS: CapBits = CapBits::FS;
const SPAWN: CapBits = CapBits::AGENT_SPAWN;
const MEM: CapBits = CapBits::MEMORY;
const NIXW: CapBits = CapBits::NIX_WRITE;
const HITL: CapBits = CapBits::HITL;
const OBS: CapBits = CapBits::OBSERVE;
const NET_FS: CapBits = CapBits::NET.union(CapBits::FS);
const MEM_HITL: CapBits = CapBits::MEMORY.union(CapBits::HITL);
const NIXW_HITL: CapBits = CapBits::NIX_WRITE.union(CapBits::HITL);
const SPAWN_FS: CapBits = CapBits::AGENT_SPAWN.union(CapBits::FS);
const NET_FS_HITL: CapBits = CapBits::NET.union(CapBits::FS).union(CapBits::HITL);
const TRANSMUTE: CapBits = CapBits::NET.union(CapBits::FS).union(CapBits::NIX_WRITE);

/// Tudo o que este binário entende. Ordem = agrupada por domínio (leitura
/// primeiro) para o prompt e a CLI serem legíveis.
pub const CATALOG: &[Capability] = &[
    // --- leitura / estado -------------------------------------------------
    c("health", "estado", NONE, false),
    c("score", "estado", NONE, false),
    c("config.show", "estado", NONE, false),
    c("cognitive.tick", "estado", NONE, false),
    c("cognitive.trace", "estado", NONE, false),
    c("artifact.validate", "estado", NONE, false),
    c("route.explain", "estado", NONE, false),
    c("self.status", "estado", NONE, false),
    c("policy.matrix", "estado", OBS, false),
    c("fleet.health", "frota", NONE, false),
    c("prefs.get", "estado", NONE, false),
    c("prefs.set", "estado", NONE, true),
    // --- memória (neural-sgdb) -------------------------------------------
    c("memory.recall", "memória", NONE, false),
    c("memory.health", "memória", NONE, false),
    c("memory.remember", "memória", MEM, true),
    c("memory.sync", "memória", MEM, true),
    c("memory.curate", "memória", MEM_HITL, true),
    c("memory.decide", "memória", MEM_HITL, true),
    // --- chat / inferência -----------------------------------------------
    c("chat", "inferência", INFER, true),
    c("chat.submit", "inferência", INFER, true),
    c("chat.poll", "inferência", INFER, false),
    c("infer.submit", "inferência", INFER, true),
    c("job.submit", "inferência", INFER, true),
    c("voice.stt", "voz", INFER, true),
    c("voice.tts", "voz", INFER, true),
    c("voice.turn", "voz", INFER, true),
    c("voice.status", "voz", NONE, false),
    c("audio.devices", "voz", NONE, false),
    c("audio.act", "voz", OBS, true),
    c("audio.volume", "voz", OBS, true),
    c("audio.device", "voz", OBS, true),
    // --- modelos ----------------------------------------------------------
    c("model.list", "modelos", NONE, false),
    c("model.suggest", "modelos", NONE, false),
    c("model.plan", "modelos", NONE, false),
    c("model.load", "modelos", INFER.union(FS), true),
    c("model.unload", "modelos", INFER, true),
    c("model.fetch", "modelos", NET_FS, true),
    c("model.search", "modelos", NET_FS, true),
    c("brain.plan", "modelos", NONE, false),
    c("brain.accept", "modelos", NET_FS_HITL, true),
    // --- domínio Nix ------------------------------------------------------
    c("nix.status", "nix", NONE, false),
    c("nix.ask", "nix", NONE, false),
    c("nix.audit", "nix", NONE, false),
    c("nix.propose", "nix", NIXW, true),
    c("nix.apply", "nix", NIXW, true),
    c("nix.rollback", "nix", NIXW, true),
    c("nix.confirm", "nix", NIXW, true),
    c("nix.build_vm", "nix", NIXW, true),
    c("transmute.status", "nix", NONE, false),
    c("transmute.run", "nix", TRANSMUTE, true),
    c("self.heal.evaluate", "nix", NIXW_HITL, true),
    // --- fábrica de apps / skills ----------------------------------------
    c("factory.plan", "fábrica", NONE, false),
    c("factory.scaffold", "fábrica", NONE, true),
    c("factory.build", "fábrica", SPAWN_FS, true),
    c("factory.promote", "fábrica", SPAWN, true),
    c("factory.seek", "fábrica", SPAWN, true),
    c("factory.synthesize", "fábrica", SPAWN, true),
    c("improve.check", "fábrica", NONE, false),
    c("improve.propose", "fábrica", NONE, false),
    c("agent.spawn", "fábrica", SPAWN, true),
    c("store.install", "skills", SPAWN, true),
    c("store.search", "skills", SPAWN, false),
    c("store.index", "skills", SPAWN, true),
    c("skill.list", "skills", NONE, false),
    c("skill.run", "skills", NONE, true),
    c("skill.explain", "skills", MEM, false),
    c("skill.select", "skills", MEM, false),
    c("skill.install", "skills", FS, true),
    c("skill.push", "skills", FS, true),
    c("skill.import", "skills", FS, true),
    // --- ferramentas / lab ------------------------------------------------
    c("mcp.list", "ferramentas", NONE, false),
    c("mcp.call", "ferramentas", NONE, true),
    c("dag.plan", "ferramentas", NONE, false),
    // --- web (o que não está no sistema vai procurar-se fora) -------------
    c("net.research", "web", NET, true),
    // --- estado distribuído ----------------------------------------------
    c("crdt.get", "frota", NONE, false),
    c("crdt.put", "frota", MEM, true),
    c("crdt.sync", "frota", MEM, true),
    c("blob.deliberate", "frota", NONE, false),
    // --- hardware / observação -------------------------------------------
    c("observe.energy", "observação", NONE, false),
    c("observe.net", "observação", NONE, false),
    c("observe.audio", "observação", NONE, false),
    c("observe.snapshot", "observação", OBS, false),
    c("hw.act", "hardware", OBS, true),
    c("gpu.plan", "hardware", OBS, false),
    c("device.recipe", "hardware", OBS, false),
    c("display.blank", "hardware", OBS, true),
    c("wifi.scan", "rede", NONE, false),
    c("wifi.connect", "rede", HITL, true),
    c("firmware.fetch", "hardware", NET_FS, true),
    // --- UI / HITL --------------------------------------------------------
    c("ui.list", "interface", NONE, false),
    c("ui.card", "interface", NONE, false),
    c("ui.pending", "interface", NONE, false),
    c("ui.open", "interface", NONE, true),
    c("ui.close", "interface", NONE, true),
    c("ui.action", "interface", HITL, true),
    c("orb.get", "interface", NONE, false),
    c("orb.set", "interface", NONE, true),
    c("orb.sense", "interface", NONE, false),
    c("orb.symbiosis", "interface", NONE, false),
    // --- segurança / sistema ---------------------------------------------
    c("system.suspend", "sistema", HITL, true),
    c("system.hibernate", "sistema", HITL, true),
    c("systemctl.scoped", "sistema", HITL, true),
    c("security.rotate", "segurança", NIXW_HITL, true),
];

/// Domínios na ordem em que aparecem no catálogo (sem repetições).
pub fn domains() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for cap in CATALOG {
        if !out.contains(&cap.domain) {
            out.push(cap.domain);
        }
    }
    out
}

/// `INFER|FS` — legível e determinístico (ordem fixa dos bits).
pub fn caps_label(caps: CapBits) -> String {
    if caps.0 == CapBits::DENY_UNKNOWN.0 {
        return "DENY_UNKNOWN".into();
    }
    if caps.0 == 0 {
        return "NONE".into();
    }
    let pairs = [
        (CapBits::INFER, "INFER"),
        (CapBits::NET, "NET"),
        (CapBits::FS, "FS"),
        (CapBits::AGENT_SPAWN, "SPAWN"),
        (CapBits::MEMORY, "MEMORY"),
        (CapBits::NIX_WRITE, "NIX_WRITE"),
        (CapBits::HITL, "HITL"),
        (CapBits::OBSERVE, "OBSERVE"),
    ];
    let mut parts: Vec<&str> = Vec::new();
    for (bit, name) in pairs {
        if caps.contains(bit) {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        format!("0x{:x}", caps.0)
    } else {
        parts.join("|")
    }
}

pub fn find(method: &str) -> Option<&'static Capability> {
    CATALOG.iter().find(|c| c.method == method)
}

/// O que o processo consegue provar de si mesmo, sem perguntar a ninguém.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SelfState {
    pub version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub node: Option<String>,
    pub nix: nix_domain::NixFacts,
}

pub fn probe() -> SelfState {
    SelfState {
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        node: std::env::var("NOSD_NODE_ID")
            .ok()
            .filter(|s| !s.trim().is_empty()),
        nix: nix_domain::probe(),
    }
}

/// Factos do runtime que só o daemon conhece (motor carregado, modelo, rede).
/// `None` = não verificado — honestidade, nunca um "sim" optimista.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct SelfEnv {
    pub engine_present: Option<bool>,
    pub model: Option<String>,
    pub net: Option<bool>,
    pub data_dir: Option<String>,
    pub memory_backend: Option<String>,
}

/// Limites **calculados** a partir dos factos: nunca uma lista estática que
/// mente quando o estado muda.
pub fn limits(st: &SelfState, env: &SelfEnv) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !st.nix.on_nixos {
        out.push(format!(
            "não estou num NixOS instalado (host={}) → sem gerações/rollback reais",
            st.os
        ));
    }
    if !st.nix.ring_armed {
        out.push(
            "ring Nix desarmado → `nix.propose/apply/confirm` recusam (root exige S-04 + ADR)"
                .into(),
        );
    }
    match env.engine_present {
        Some(false) => out.push(
            "motor S2 ausente → chat local falha fechado (EngineAbsent), sem resposta inventada"
                .into(),
        ),
        None => out.push("motor S2 não verificado neste contexto (não assumir que existe)".into()),
        Some(true) => {}
    }
    if env.net == Some(false) {
        out.push("rede indisponível → `net.research`/fetch recusam em vez de fingir".into());
    }
    out.push(
        "identidade não vem do payload: Unix = SO_PEERCRED, TCP/HTTP = token; `caps`/`agent` são legado"
            .into(),
    );
    out.push(
        "HITL é server-authored (nonce de uso único + TTL): `hitl_confirmed` do cliente não autoriza"
            .into(),
    );
    out
}

/// Como é feito por dentro — os invariantes que um agente tem de respeitar
/// para não partir o sistema.
pub const ENTRAILS: &[&str] = &[
    "transporte: Unix socket (SO_PEERCRED) ou TCP/HTTP loopback com token (<data-dir>/ipc.token)",
    "caps: NONE=lê, INFER=modelo, MEMORY=sgdb, NET=rede, FS=disco, SPAWN=subprocesso, NIX_WRITE=root, HITL=decisão humana",
    "política: fail-closed — método desconhecido, parse falhado ou identidade ausente = deny",
    "memória: neural-sgdb em <data-dir>/memory/nsgdb (scopes aios/*, nsgdb/doctrine) — consultiva, nunca expressão",
    "estado distribuído: CRDT LWW+VV em <data-dir>/mesh-crdt (log assinado + âncora anti-rollback)",
    "root: helper Nix separado, target fixo, staging confinado, lock e timeout — nunca no chat",
    "efeitos: OPAR regista acções com side-effect (auditoria barata)",
    "evidência: `docs/evidence/` + `Tested:`/`Not-tested:` no commit — QEMU ≠ metal",
];

/// Onde procurar o quê. Um agente que não saiba isto inventa caminhos.
pub const POINTERS: &[(&str, &str)] = &[
    (
        "flake.nix",
        "inputs, checks, packages, devShell — a árvore git é a fonte",
    ),
    (
        "modules/*.nix",
        "módulos NixOS: nosd, nosd-engine, cage, transmute, nix-ring",
    ),
    (
        "images/*.nix",
        "ISO/QEMU/HW: iso.nix (release), hw.nix (metal), iso-demo",
    ),
    (
        "packages/nosd/crates/",
        "workspace Rust: nosd, nos-core, nos-hermes, nos-cortex, nos-memory, nos-ipc",
    ),
    ("docs/API.md", "métodos IPC, caps e contratos"),
    ("docs/SECURITY.md", "ameaças, S-04/S-07, ring, HITL"),
    (
        "docs/evidence/",
        "o que foi OBSERVED em metal/QEMU (e quando)",
    ),
    (
        "AGENTS.md",
        "regras não negociáveis + lições que custaram builds",
    ),
    (
        "docs/MEMORY_NSGDB.md",
        "scopes, writers e a lei consultiva do substrato",
    ),
];

fn one_line(s: &str, max: usize) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    t.chars().take(max).collect()
}

/// Bloco `## Self (live)` — o que o modelo sabe de si antes de responder.
///
/// Tecto duro: um 0.8B não aguenta um manual. Domínios e contagens vêm do
/// catálogo (não podem mentir); os detalhes ficam em `self.status`/CLI/MCP.
pub fn prompt_block(st: &SelfState, env: &SelfEnv, max_chars: usize) -> String {
    let mut s = String::from("## Self (live)\n");
    let node = st.node.as_deref().unwrap_or("sem NOSD_NODE_ID");
    let motor = match env.engine_present {
        Some(true) => "s2 carregado",
        Some(false) => "ausente",
        None => "não verificado",
    };
    s.push_str(&format!(
        "identidade: NixAIOS {} ({}/{}) | nó {node} | memória {} | motor: {motor}\n",
        st.version,
        st.os,
        st.arch,
        env.memory_backend.as_deref().unwrap_or("não verificada")
    ));
    let doms = domains();
    let dom_txt: Vec<&str> = doms.iter().take(10).copied().collect();
    s.push_str(&format!(
        "entranhas: {} métodos em {} domínios ({}{}); caps NONE=lê INFER=modelo MEMORY=sgdb NET=rede FS=disco NIX_WRITE=root HITL=humano\n",
        CATALOG.len(),
        doms.len(),
        dom_txt.join(", "),
        if doms.len() > dom_txt.len() { "…" } else { "" }
    ));
    s.push_str(
        "posso: ler estado/nix/memória/energia/trace; agir com factory.scaffold|build, memory.remember, \
         skill.install|run, model.load, transmute.run, net.research, ui.open\n",
    );
    let lim = limits(st, env);
    let lim_txt: Vec<String> = lim.iter().take(2).map(|l| one_line(l, 120)).collect();
    s.push_str(&format!("limites: {}\n", lim_txt.join("; ")));
    s.push_str(
        "onde: flake.nix, modules/*.nix, images/*.nix, packages/nosd/crates/, docs/API.md, docs/evidence/\n",
    );
    s.push_str("fora do sistema: net.research (https, cap NET) e citar a fonte; nunca inventar\n");
    if s.chars().count() > max_chars {
        s = s.chars().take(max_chars).collect();
    }
    s
}

/// `self.status`: identidade + capacidades + entranhas + ponteiros.
pub fn status(st: &SelfState, env: &SelfEnv) -> serde_json::Value {
    let caps: Vec<serde_json::Value> = CATALOG
        .iter()
        .map(|c| {
            serde_json::json!({
                "method": c.method,
                "domain": c.domain,
                "caps": caps_label(c.caps),
                "side_effect": c.side_effect,
            })
        })
        .collect();
    serde_json::json!({
        "identity": {
            "name": "NixAIOS",
            "version": st.version,
            "os": st.os,
            "arch": st.arch,
            "node": st.node,
            "nix": st.nix,
        },
        "runtime": env,
        "capabilities": caps,
        "counts": {
            "methods": CATALOG.len(),
            "domains": domains().len(),
            "read_only": CATALOG.iter().filter(|c| !c.side_effect).count(),
            "side_effect": CATALOG.iter().filter(|c| c.side_effect).count(),
        },
        "domains": domains(),
        "limits": limits(st, env),
        "entrails": ENTRAILS,
        "pointers": POINTERS.iter().map(|(k, v)| serde_json::json!({"where": k, "what": v})).collect::<Vec<_>>(),
        "honesty": "capacidades = mesma tabela que autoriza (teste de contrato em nos-ipc); ausente = null",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_is_unique_and_complete_enough() {
        let mut seen = BTreeSet::new();
        for cap in CATALOG {
            assert!(seen.insert(cap.method), "método duplicado: {}", cap.method);
            assert!(!cap.domain.is_empty(), "{} sem domínio", cap.method);
            assert!(
                cap.caps.0 != CapBits::DENY_UNKNOWN.0,
                "{} não pode ser DENY_UNKNOWN",
                cap.method
            );
        }
        // Um nó que "sabe quem é" tem de conhecer os métodos que anunciam o
        // próprio estado e o domínio Nix que o AGENTS exige.
        for must in [
            "health",
            "self.status",
            "nix.status",
            "nix.ask",
            "memory.recall",
            "factory.build",
            "net.research",
        ] {
            assert!(find(must).is_some(), "catálogo sem {must}");
        }
        assert!(
            CATALOG.len() >= 90,
            "catálogo curto demais: {}",
            CATALOG.len()
        );
    }

    #[test]
    fn caps_label_is_deterministic() {
        assert_eq!(caps_label(CapBits::NONE), "NONE");
        assert_eq!(
            caps_label(CapBits::HITL.union(CapBits::MEMORY)),
            "MEMORY|HITL"
        );
        assert_eq!(caps_label(CapBits::DENY_UNKNOWN), "DENY_UNKNOWN");
    }

    #[test]
    fn limits_follow_the_facts_not_optimism() {
        let st = SelfState {
            version: "test",
            os: "linux",
            arch: "x86_64",
            node: None,
            nix: nix_domain::NixFacts::default(),
        };
        let env = SelfEnv {
            engine_present: Some(false),
            ..Default::default()
        };
        let l = limits(&st, &env).join(" | ");
        assert!(l.contains("ring Nix desarmado"), "{l}");
        assert!(l.contains("motor S2 ausente"), "{l}");
        assert!(l.contains("não estou num NixOS"), "{l}");

        let env_ok = SelfEnv {
            engine_present: Some(true),
            ..Default::default()
        };
        let l2 = limits(&st, &env_ok).join(" | ");
        assert!(!l2.contains("motor S2 ausente"), "{l2}");
    }

    #[test]
    fn prompt_block_respects_cap_and_has_identity() {
        let st = probe();
        let env = SelfEnv::default();
        let b = prompt_block(&st, &env, 700);
        assert!(b.chars().count() <= 700);
        assert!(b.starts_with("## Self (live)"));
        assert!(b.contains("identidade: NixAIOS"));
        assert!(b.contains("métodos em"));
        assert!(b.contains("net.research"));
    }

    #[test]
    fn status_exposes_every_capability() {
        let st = probe();
        let v = status(&st, &SelfEnv::default());
        let caps = v["capabilities"].as_array().unwrap();
        assert_eq!(caps.len(), CATALOG.len());
        assert_eq!(
            v["counts"]["methods"].as_u64().unwrap() as usize,
            CATALOG.len()
        );
        assert!(!v["limits"].as_array().unwrap().is_empty());
    }
}
