# NixAIOS 0.5.6 — QEMU green (R-09)

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.5.6  
**Data:** 2026-10-26  
**Tag:** `v0.5.6`  
**Tema:** ISO volta a construir no nixpkgs atual e arranca em QEMU com `nosd`.

## O que mudou

- **nixpkgs 26.11:** `substituteAll` foi removido; `modules/transmute-stage.nix`
  passa a usar `replaceVars` + `runCommand` (hotswap/firstboot).
- **UTF-8:** `modules/nosd-early.nix` tinha um byte cp1252 inválido (0x97);
  normalizado.
- **CRLF:** `.nix`/`.sh` normalizados para LF (flake.nix incluído); scripts
  gerados por `writeShellScriptBin` deixam de falhar com `$'\r'`.
- **QEMU serial:** `images/iso.nix` adiciona `console=ttyS0,115200n8` para
  capturar boot headless.
- **Evidência:** [`evidence/QEMU_0.5.6.md`](evidence/QEMU_0.5.6.md) — ISO
  construída, `nosd` Started, `nosd-early` Finished, JARBAS hub, multi-user,
  gate MQEMU PASS.
- **R-05:** `checks.x86_64-linux.nixaios-vm` sobe NixOS e valida
  `nosctl health` + `nosctl chat oi` (61s).
- **Q-03:** CI GitHub (`lint`, `test` Linux/Windows, `nix-vm`; ISO on-demand).

## Build

```bash
nix build .#iso --print-out-paths
# PVD: NIXAIOS051
```

## Residual (honesty)

- QEMU ≠ metal; M4 continua AWAITING_HW.
- Chat/GGUF, voz, cage/DRM e Wi-Fi não foram exercitados neste boot.
- Firstboot tty1 não dispara em ttyS0 (lab); M2/M3b precisam de interação/ajuste.
- S-04 completo (remover legado admin), S-08/S-09, R-05 VM test, Q-03 CI.
