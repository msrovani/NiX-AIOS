# Changelog

Formato inspirado em [Keep a Changelog](https://keepachangelog.com/).  
Versionamento [SemVer](https://semver.org/).

## [0.5.1] — 2026-10-25

### Added

- Documentação moderna: hub [`docs/README.md`](docs/README.md), ARCHITECTURE, GETTING_STARTED, DEVELOPMENT, API, OPERATIONS, SECURITY, GLOSSARY, ROADMAP.
- Raiz: CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, LICENSE (AGPL-3.0), CHANGELOG.
- [`docs/RELEASE_0.5.1.md`](docs/RELEASE_0.5.1.md); ISO volid `NIXAIOS051`.

### Changed

- Workspace / flake version → `0.5.1`.
- Empacota e publica em Git o fecho técnico 0.5.0 (nunca tagged isolado).

### Notes

Funcionalidade metal/Linux-first documentada em [`docs/RELEASE_0.5.0.md`](docs/RELEASE_0.5.0.md).

## [0.5.0] — 2026-10-25 (código; ship Git = 0.5.1)

### Fixed

- Metal pós-Q5: `resolve_models_dir` **Linux-first** (evita EACCES quando FAT/Landlock desalinhados).
- Landlock allowlist inclui `/mnt/nixaios-log` (defesa se FAT opt-in).
- Firstboot: `save_file`/`set_step` falham de forma visível; erros `transmute` impressos no tty.
- Cage/ISO: permissões `1777` em `/var/lib/nosd` (sem `chmod 750` agressivo no pre-start).
- Paths de voz/locale sem preferência FAT.

### Added

- `NIXAIOS_USE_FAT_PERSIST=1` + probe de escrita.
- CRDT thin LWW + version vector (`nos-mesh::crdt`).
- `SoulMirrorSnapshot` + EventBus `SOUL_MIRROR`.
- `llama_server_posture()` honesty (S2.2).
- `evaluate_self_heal` CapGate Deny-by-default.
- `scripts/build-iso-050.sh`, `scripts/smoke-transmute-paths.sh`.

### Security

- CapGate server-authored (I.1); S1.2 no-seed; guard ISO.

### Honesty / residual

- Boot metal / lab 2-nó / CUDA VERIFIED — fora do aceite.

## [0.1.0] — 2026-10-21 … 2026-10-25

Scaffold NixAIOS (ADR-0118), remigração s529. Ver ledger de remigração.
