# Documentação NixAIOS

Hub da documentação do produto paralelo **NixAIOS** (daemon `nosd` + ISO NixOS).  
Versão corrente: **0.6.0-alpha.11** — [`RELEASE_0.6.0-alpha.11.md`](RELEASE_0.6.0-alpha.11.md) (base [`RELEASE_0.5.1.md`](RELEASE_0.5.1.md) / [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md)).

> Bare-metal canónico: [neural-os-core](https://github.com/msrovani/neural-os-core). Este repo = bancada NixOS (ADR-0118).

## Começar aqui

| Documento | Para quem |
|-----------|-----------|
| [GETTING_STARTED.md](GETTING_STARTED.md) | Primeiro contacto (host TCP, WSL, ISO) |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Visão de sistema, crates, camadas PORT/ADAPT |
| [API.md](API.md) | `nosctl` / IPC JSON |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Build, testes, convenções |
| [SECURITY.md](SECURITY.md) | CapGate, Landlock, threat model |
| [OPERATIONS.md](OPERATIONS.md) | ISO, metal, journal, firstboot |
| [GLOSSARY.md](GLOSSARY.md) | Termos (Hermes, JARBAS, Frugal, …) |
| [ROADMAP.md](ROADMAP.md) | Residuals pós-0.5.0 e honesty |
| [IMPROVEMENT_PLAN.md](IMPROVEMENT_PLAN.md) | Plano de melhorias por ondas (0.5.4 → 0.6/0.7/0.8) |
| [BOOT_TO_JARBAS.md](BOOT_TO_JARBAS.md) | Playbook M0–M5 boot→JARBAS Jarvis-like + rings + AI_GATE |
| [METAL_CAMPAIGN.md](METAL_CAMPAIGN.md) | Campanha M4 stick NIXAIOS051 (Ring3_Linux/metal) |
| [FLEET_LAB.md](FLEET_LAB.md) | Campanha M5 mesh 2-nó (0081 A/B) |
| [SKILL_TRUST_AND_SELECTION.md](SKILL_TRUST_AND_SELECTION.md) | Requisitos de segurança/qualidade das skills wasm e escolha pelo Hermes |
| [SKYNET_DISTRIBUTED_PLAN.md](SKYNET_DISTRIBUTED_PLAN.md) | Plano da frota distribu?da (M6-M10): identidade, Noise, jobs, CRDT, wasmi, segurança |

## Memoria operacional

| Documento | Função |
|-----------|--------|
| [memory/SESSION_FLEET_M6_M9.md](memory/SESSION_FLEET_M6_M9.md) | Lições da sessão frota (M6→S2b) e estado à saída |

## Honesty e remigração
## Honesty e remigração

| Documento | Função |
|-----------|--------|
| [TECNOLOGIAS_NIXAIOS.md](TECNOLOGIAS_NIXAIOS.md) | Inventário PORT/ADAPT com status honesto |
| [MIGRATION_MATRIX.md](MIGRATION_MATRIX.md) | Matriz bare → NixOS |
| [REMIGRATION_PLAN_LEDGER.md](REMIGRATION_PLAN_LEDGER.md) | 55 itens DONE/PARTIAL/NOT_DONE |
| [adr-addenda/](adr-addenda/README.md) | Addenda ADR-0118 |

**Regra ADR-0113:** scaffold ≠ VERIFIED; QEMU/ISO PASS ≠ metal PASS. UI “todo completed” ≠ aceite.

## Bancada e stick

| Documento | Função |
|-----------|--------|
| [WSL.md](WSL.md) | Setup WSL2 + Nix |
| [stick-log-fat32.md](stick-log-fat32.md) | Log FAT (legado; 0.5.0 default OFF) |
| [stick-copy-journal-usb.md](stick-copy-journal-usb.md) | Journal → USB |
| [cortex-nix-ring.md](cortex-nix-ring.md) | Anel `nix.propose` / `nix.apply` |

## Raiz do repositório

- [README.md](../README.md) — visão rápida  
- [CONTRIBUTING.md](../CONTRIBUTING.md) — contribuição  
- [CHANGELOG.md](../CHANGELOG.md) — histórico de releases  
- [SECURITY.md](../SECURITY.md) — reporting (ponte para docs/)
