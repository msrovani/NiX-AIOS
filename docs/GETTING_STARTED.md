# Getting started — NixAIOS 0.5.6

Três bancadas, três níveis de honestidade (ADR-0113):

| Bancada | O que prova |
|---------|-------------|
| Host Windows TCP | IPC / chat echo / testes unitários |
| WSL + Nix | smoke cognitivo, ISO build |
| Metal USB | boot real — **AWAITING_HW** até evidência |

## 1) Host rápido (Windows)

Requisitos: Rust stable, sem Nix obrigatório.

```powershell
cd C:\DEV\nix-aios\packages\nosd
cargo test --workspace
cargo run -p nosd -- --foreground --tcp 127.0.0.1:7700 --data-dir ./data
```

Noutro terminal:

```powershell
# TCP exige token (S-01); o daemon grava ./data/ipc.token
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 --token-file ./data/ipc.token health
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 --token-file ./data/ipc.token chat "oi"
```

Sem GGUF/`llama-completion` no PATH → resposta `[cpu-echo]` (honesto).

Lab orb HTTP (se activo): `http://127.0.0.1:7701/`.

## 2) WSL + Nix

Seguir [`WSL.md`](WSL.md). Resumo:

```bash
cd /path/to/nix-aios   # ou nixaios/
nix develop
nix build .#nosd
bash scripts/smoke-cognitive-loop.sh
bash scripts/smoke-transmute-paths.sh
```

ISO 0.5.x (volid `NIXAIOS051`):

```bash
bash scripts/build-iso.sh
# Artefactos típicos: Downloads/nixaios-minimal-051*.iso — volid NIXAIOS051
```

Seek/AutoLearn no tick contínuo: só com `NOSD_AUTOLEARN=1` (default off).

## 3) Metal (USB)

1. Rufus **DD** (não modo ISO residual).
2. Preferir copytoram se a média for flaky.
3. Autologin → questionário texto (Q1 rede … Q5 confirmar).
4. Pós-Q5: `nosctl transmute` → hotswap → JARBAS em tty7 (Alt+F7).
5. Logs: `/tmp/nos-firstboot.log`, `/var/lib/nosd/firstboot.log`, `journalctl -u nosd -u jarbas-cage`.

Preflight: `scripts/metal-iso-deep-preflight.sh`, handoff `scripts/metal-usb-handoff.ps1`.

## Variáveis úteis

| Env | Efeito |
|-----|--------|
| `NOSD_LANDLOCK=1` | Landlock no Linux (unit ISO já liga) |
| `NOSD_LLAMA_CLI` / `NOSD_LLAMA_NGL` | Engine GGUF + offload |
| `NOSD_LLAMA_SERVER` | URL llama-server (S2.2 posture) |
| `NIXAIOS_USE_FAT_PERSIST=1` | Modelos em FAT (opt-in) |
| `NOSD_S1_BACKEND=keyword` | S1 sem blob (default ISO) |
| `NOSD_MEMORY_BACKEND=nsgdb` | Default `seg.bin` |

## Próximos passos

- API: [`API.md`](API.md)  
- Dev: [`DEVELOPMENT.md`](DEVELOPMENT.md)  
- Ops metal: [`OPERATIONS.md`](OPERATIONS.md)
