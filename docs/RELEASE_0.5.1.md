# NixAIOS 0.5.1 — Release notes

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.5.1  
**Data:** 2026-10-25

## O que é esta release

Primeira publicação Git desta linha 0.5.x: inclui o fecho técnico **0.5.0** (metal pós-Q5 Linux-first) mais a **documentação moderna** e versionamento explícito 0.5.1.

| Base | Conteúdo |
|------|----------|
| 0.5.0 (código) | Persist Linux-first, Landlock FAT, firstboot fail-loud, CRDT/SoulMirror, SelfHeal stub — [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md) |
| 0.5.1 (este) | Hub `docs/`, CONTRIBUTING, SECURITY, CHANGELOG, LICENSE AGPL; volid `NIXAIOS051` |

## Garantias (inalteradas)

CapGate server-authored · S1.2 no-seed ISO · Frugal wasmi · Seek-before-generate · `autoSwitch=false` · honesty ADR-0113.

## Documentação

Entrada: [`README.md`](README.md) (hub).  
Raiz: CONTRIBUTING, CHANGELOG, SECURITY, CODE_OF_CONDUCT, LICENSE.

## Build

```bash
bash scripts/build-iso-050.sh   # produz volid NIXAIOS051
```

## Residual

Metal boot VERIFIED, lab 2-nó, CUDA VERIFIED — AWAITING_HW / fora do aceite.
