# Lab M5 — Fleet 2-nó (SoulMirror / offload)

**ring=`Ring3_Linux/fleet`** — **não** Ring0. IDEA **#707**.  
ADR-**0081** fases A/B. Playbook: [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md).

## GOAL

Dois nós NixAIOS (lab) com handshake SoulMirror e offload S2 opcional — sem auto-pairing prod.

## Fases ADR-0081

| Fase | Escopo | Gate |
|------|--------|------|
| **A** | Handshake + identity mirror (lab) | 2 peers reachability + mirror ack |
| **B** | Offload S2 deliberado (CapGate) | route S2 → peer; local S1 intacto |

## Procedimento (lab)

1. Dois hosts (ou VM+metal) com `nosd` + mesh enabled.
2. Config peer allowlist **manual** (sem discovery aberto).
3. `bash scripts/gate-boot-to-jarbas.sh --stage M5` (docs + claim honesty).
4. Evidência: logs handshake + `route.explain` S2 offload (se B).

## OK / LOST

| OK | LOST |
|----|------|
| Fase A ack documentado | “mesh prod-ready” sem allowlist |
| Fase B CapGate + S1 local | auto-pairing / claim Ring0 |
| claim `Ring3_Linux/fleet` | conflar com metal VERIFIED |

## Status

**AWAITING_HW / lab** — estrutura documental; execução peer-a-peer pendente.
