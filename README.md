# NixAIOS

> **Procedencia:** importado de [msrovani/neural-os-core](https://github.com/msrovani/neural-os-core) commit `bfd03f4a` (arvore `nixaios/`) em 2026-10-23. O bare-metal continua canonico la; este repo e a bancada NixOS (ADR-0118, addendum de mudanca de repositorio: https://github.com/msrovani/neural-os-core/blob/main/docs/architecture/0118-nixaios-nosd.md).


Produto **paralelo** ao neural-os-core bare-metal (não é fork). ADR-0118.

**Plano remigração (s529):** status item-a-item em [`docs/REMIGRATION_PLAN_LEDGER.md`](docs/REMIGRATION_PLAN_LEDGER.md) — DONE/PARTIAL/NOT_DONE dos 55 todos. UI “completed” ≠ aceite.

## Quick start (Windows / WSL)

```bash
cd nixaios/packages/nosd
cargo test --workspace
# Windows TCP lab:
cargo run -p nosd -- --foreground --tcp 127.0.0.1:7700 --data-dir ./data
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 health
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 chat "oi"
```

Sem GGUF/`llama-completion` → `[cpu-echo]`. Loop cognitivo WSL:

```bash
bash nixaios/scripts/smoke-cognitive-loop.sh   # PASS = tokens + remember + recall
```

WASM: ADR-**0059** (não 0032). Frugal: 256 KiB = **4** páginas / fuel 100k.  
QEMU PASS ≠ metal PASS (ADR-0113). WSL: [docs/WSL.md](docs/WSL.md).  
CUDA: `smoke-cuda-honesty.sh` — host GPU OBSERVED ≠ `CUDA_ENGINE_ABSENT` (nixpkgs llama CPU); opt-in `nix build .#llama-cpp-cuda`.  
Metal: `metal-iso-deep-preflight.sh` / `metal-usb-handoff.ps1` → Rufus **DD** + copytoram; boot = AWAITING_HW.

## Nix

```bash
cd nixaios
nix develop
nix build .#nosd
# ISO / CUDA opt-in (WSL):
# nix build .#iso
# nix build .#llama-cpp-cuda   # pesado; só p/ NOSD_LLAMA_NGL>0 VERIFIED
```

**Display / persona:** UI = **JARBAS** (cerimónia) → **Hermes** → **Cortex**.  
Janelas = objetos HITL/mostrar (ex. Wi‑Fi+senha), não painel sysadmin.  
Wayland+`cage` = transporte DRM (tty7, fail-closed). Ver `0118-jarbas-persona-ui.md` + `0118-wayland-cage-kiosk.md`.

**Boot auto:** `nosd` → `jarbas-hub` (journal) → `jarbas-cage` (tty7, não bloqueia boot).  
Health: `hermes=running`. `journalctl -u nosd -u jarbas-cage`.

**HotSwap / HF (GGUF MVP):** probe → `nosctl suggest` → `fetch --repo … --file …` → `load PATH` / `unload`.  
Engine path A = `llama-cli` subprocess; EXL2/vLLM = `EngineAbsent` até flake. CapGate `NET|FS` no fetch.  
Ver `docs/adr-addenda/0118-hotswap-hf.md`.

## Layout

- `packages/nosd/` — workspace Rust (`nosd`, hub, factory, wasm Frugal, memory/nsgdb facade, mesh, BEI, hermes, jarbas, voice)
- `modules/` — systemd nosd, cage iGPU, ai-managed.nix
- `docs/` — MIGRATION_MATRIX, TECNOLOGIAS_NIXAIOS, WSL
