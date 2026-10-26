# Documentação NixAIOS

Hub da documentação do produto paralelo **NixAIOS** (daemon `nosd` + ISO NixOS).  
Versão corrente: **0.5.2** — [`RELEASE_0.5.2.md`](RELEASE_0.5.2.md) (base [`RELEASE_0.5.1.md`](RELEASE_0.5.1.md) / [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md)).

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
