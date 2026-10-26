# NixAIOS 0.5.0 — Release notes

**Volid ISO:** `NIXAIOS050`  
**Workspace:** `packages/nosd` 0.5.0  
**Foco:** migração metal pós-Q5 (persistência Linux-first) + fecho honesty da remigração.

## Garantias desta release

| Garantia | Evidência |
|----------|-----------|
| CapGate server-authored (client ALL ignored) | I.1 host VERIFIED |
| S1.2 — ISO **não** embute blobs S1/AgentJev/Piper seed | `guard-iso-no-s1-blob.sh` no build; `blob_deliberate` runtime |
| Frugal wasmi 256 KiB / fuel 100k (path A) | ADR-0059; smoke-frugal-fleet |
| Seek-before-generate antes de synth | F.3 host VERIFIED |
| SITE_POLICY ISO `autoSwitch=false` | NX.0 |
| Persistência default = `/var/lib/nosd` (não FAT) | `resolve_models_dir` Linux-first; FAT só `NIXAIOS_USE_FAT_PERSIST=1` |
| Honesty ADR-0113 | scaffold ≠ VERIFIED; QEMU/ISO PASS ≠ metal PASS |

## Correcção P0 (metal pós-Q5)

Questionário Q1–Q4 gravava; Q5/`transmute` falhava com **permission denied** quando o daemon preferia FAT ou Landlock bloqueava `/mnt/nixaios-log`.

- Modelos → `data_dir/models` por omissão.
- Landlock allowlist inclui `/mnt/nixaios-log` (defesa opt-in).
- Cage/ISO: `1777` em `/var/lib/nosd` (sem `chmod 750` no pre-start).
- Firstboot: `save_file` fail-loud; erros `transmute` no tty.

## Residuals explícitos (não overclaim)

- Boot metal VERIFIED — AWAITING_HW nesta sessão.
- Lab mesh 2-nó físico, CUDA `.#llama-cpp-cuda` VERIFIED, EXL2/vLLM.
- CRDT mesh / SoulMirror: código+teste unitário; e2e lab residual.

Ver [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md).

## Build

```bash
bash scripts/build-iso-050.sh   # WSL; guard S1.2 obrigatório
```
