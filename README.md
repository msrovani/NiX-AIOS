# NixAIOS

> **Procedência:** importado de [msrovani/neural-os-core](https://github.com/msrovani/neural-os-core) commit `bfd03f4a` (árvore `nixaios/`) em 2026-10-23. O bare-metal continua canónico lá; este repo é a bancada NixOS ([ADR-0118](https://github.com/msrovani/neural-os-core/blob/main/docs/architecture/0118-nixaios-nosd.md)).

Produto **paralelo** ao neural-os-core (não é fork). Licença **AGPL-3.0**.

**Release 0.6.0-alpha.2** — GGUF real no VM (llama-server); volid ISO `NIXAIOS051`.  
**Documentação:** [`docs/README.md`](docs/README.md) · [Getting started](docs/GETTING_STARTED.md) · [Arquitectura](docs/ARCHITECTURE.md) · [API](docs/API.md) · [Changelog](CHANGELOG.md) · [Release 0.6.0-alpha.2](docs/RELEASE_0.6.0-alpha.2.md)

## Quick start

```bash
cd packages/nosd
cargo test --workspace
cargo run -p nosd -- --foreground --tcp 127.0.0.1:7700 --data-dir ./data
# TCP exige token (S-01); o daemon grava ./data/ipc.token
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 --token-file ./data/ipc.token health
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 --token-file ./data/ipc.token chat "oi"
```

Sem GGUF → `[cpu-echo]`. WSL / ISO / metal: ver [Getting started](docs/GETTING_STARTED.md) e [WSL](docs/WSL.md).

```bash
bash scripts/smoke-cognitive-loop.sh
bash scripts/build-iso-050.sh          # WSL; guard S1.2
```

## Stack (resumo)

| Camada | Peças |
|--------|--------|
| Daemon | `nosd` Tokio + CapGate + Landlock |
| Agente | Hermes OPAR → Cortex HotSwap GGUF |
| UI | JARBAS Slint + Wayland `cage` (tty7) |
| Memória / malha | nsgdb `seg.bin`, mesh UDP, CRDT thin |
| NixOS | flake + modules + ISO minimal firstboot |

**Honesty (ADR-0113):** scaffold ≠ VERIFIED; QEMU/ISO PASS ≠ metal PASS.  
Ledger: [`docs/REMIGRATION_PLAN_LEDGER.md`](docs/REMIGRATION_PLAN_LEDGER.md).

## Layout

```
packages/nosd/   workspace Rust
modules/         systemd nosd, cage, transmute, nix-ring
images/iso.nix   ISO NIXAIOS051
scripts/         build, smokes, metal
docs/            documentação moderna + ADRs + honesty
```

## Contribuir e segurança

- [CONTRIBUTING.md](CONTRIBUTING.md) · [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- [SECURITY.md](SECURITY.md) · [docs/SECURITY.md](docs/SECURITY.md)
- [LICENSE](LICENSE) (AGPL-3.0)
