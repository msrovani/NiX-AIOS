# NixAIOS 0.5.2 — Release notes

**Volid ISO:** `NIXAIOS051` (inalterado; SemVer patch no workspace)  
**Workspace:** `packages/nosd` 0.5.2  
**Data:** 2026-10-25

## O que é esta release

Revisão estrutural/arquitetural completa (higiene + contratos runtime + refactors) sobre a base 0.5.1. Wire IPC estável — patch SemVer.

| Onda | Conteúdo |
|------|----------|
| Hygiene | Scripts s5xx → `scripts/archive/`; canónico `scripts/build-iso.sh`; docs volid `NIXAIOS051`; Landlock soft + `ReadWritePaths` condicional |
| Contratos | IPC registry test; remove stub `mesh.skillsync`; `NOSD_AUTOLEARN` default off; hub único (`hermes.hub`); `self.heal.evaluate`; `fnv1a64` em nos-core; CRDT no SoulMirror tick |
| Estrutura | `HermesEngine::boot` + mesh/SoulMirror helpers; RPC fatiado `crates/nosd/src/rpc/`; firstboot/hotswap externos em `modules/transmute/` |

## Garantias (inalteradas)

CapGate server-authored · S1.2 no-seed ISO · Frugal wasmi · Seek-before-generate · `autoSwitch=false` · honesty ADR-0113.

## Build

```bash
bash scripts/build-iso.sh   # volid NIXAIOS051
# alias: scripts/build-iso-050.sh → build-iso.sh
```

## Env relevante

| Var | Default | Nota |
|-----|---------|------|
| `NOSD_AUTOLEARN` | off (`0`) | Seek/AutoLearn só com `=1` (lab opt-in) |

## Residual (honesty)

Metal VERIFIED, lab 2-nó físico, CUDA VERIFIED, CRDT P2P NSGDB pleno — fora de escopo / AWAITING_HW.
