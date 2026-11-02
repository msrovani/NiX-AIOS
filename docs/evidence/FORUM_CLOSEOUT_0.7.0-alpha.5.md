# Evidence — fórum closeout (0.7.0-alpha.5 in-tree)

**Data:** 2026-11-02. **Versão:** `0.7.0-alpha.5` (sem bump de pacote; closeout
do backlog OPCODE/1). **Commits:** `2d2fb01` (código) + `da0100c` (ledger)
+ commit de documentação deste artefacto.

## O que fechou

| Id | Item | Prova |
|---|---|---|
| F9/F10 | `scripts/metal-m3b-harvest.sh` sumário `A6=play_ok` + `snd_denied`, `journalctl --since`, `trap` | `bash -n` PASS |
| N1 | `curate` expire/decay/consolidate → `ok:false` + `performed:false`; RPC espelha | `curate_host_side_ops_not_performed` PASS |
| N2 | `MergeVerdict::Duplicate` ≠ `applied` (campo `duplicate`) | `telepathy_duplicate_not_counted_as_applied` PASS |
| M1 | telepathy rejeita doutrina + cap 256 (já dirty; entrou em `2d2fb01`) | `telepathy_rejects_*` PASS |
| M3b-audio | estágio opt-in (`NIXAIOS_GATE_AUDIO=1` / `--stage M3b-audio`), fora de `REQUIRED` | SKIP AWAITING_HW, `fails=0` |
| Fórum | `FORGE-0087` fecha fila; validate ledger ok | `forum-log.py validate` ok |

## Já feito antes (não reaberto)

- F4 `dispatch_blocking` (IPC + HTTP kiosk)
- F7 harvest CLI (`voice-tts` / `voice-turn --prompt`)
- Matriz Nix 2VM (nosd/iso/MQEMU/nixaios-vm/mesh-2vm/crdt/jobs) — artefactos em
  `docs/evidence/artifacts/nixaios-vm-20261102/`

## Not-tested / AWAITING_HW

- Metal Core7 colheita com harvest novo (OBSERVED com 1 boot; VERIFIED = 3+)
- `checks.x86_64-linux.mesh-3vm-faults`
- `iso-hw-cuda` / img 12G
- Rebuild Nix da matriz nesta sessão de closeout

## Próximos passos (ordem)

1. Push destes commits (se ainda não no remoto).
2. Boot metal Core7 + `metal-m3b-harvest.sh` → `docs/evidence/hw/<ts>/`.
3. Opcional: gate `M3b-audio` no metal.
4. Lab: `mesh-3vm-faults`; disco: `iso-hw-cuda`.
5. Tranche HUMAN nova para ADR voz opção A / `nix.propose`→HITL / frota multi-host.

## Refs

- Fórum: `FORGE-0086` (fim loop) → `FORGE-0087` (closeout)
- Plano: `forum_backlog_closeout` (Cursor)
- Memória sessão: `docs/memory/SESSION_20261102_FORUM_CLOSEOUT.md`
