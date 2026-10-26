# Campanha M4 — Metal stick NIXAIOS051

**ring=`Ring3_Linux/metal`** — **não** Ring0. IDEA **#707**.  
Playbook: [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md). Ledger: [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md).

## GOAL

USB live com volid `NIXAIOS051`: firstboot Q1–Q5 → transmute → hotswap → `jarbas-cage` tty7 sem cascata EACCES.

## Procedimento

1. Build: `bash scripts/build-iso.sh` (alias `build-iso-050.sh`).
2. Preflight software: `bash scripts/metal-iso-deep-preflight.sh` — PASS aqui **≠** metal VERIFIED.
3. Gravar stick: Rufus **DD** (não ISO residual); preferir copytoram se média flaky (`scripts/metal-usb-handoff.ps1`).
4. Boot → autologin → firstboot texto → Q5 confirmar.
5. Evidência:
   - `journalctl -u nosd -u jarbas-cage -u nosd-early-score -b --no-pager | tee metal-boot.log`
   - `ls -la /var/lib/nosd/{firstboot.done,hotswap.log,boot-score.json,models,nsgdb}`
   - `bash scripts/gate-boot-to-jarbas.sh --stage M4`
6. Anexar log ao ledger com `claim_status=VERIFIED` **só** se cage+nosd+firstboot.done OK sem EACCES cascata.

## OK / LOST

| OK | LOST |
|----|------|
| journal + firstboot.done + cage active | emergency midia; gui_failed permanente |
| gate M4 PASS + log anexado | só preflight software → **AWAITING_HW** |
| label ledger `Ring3_Linux/metal` | claim “Ring0 VERIFIED” |

## Status actual (honesty)

**AWAITING_HW** — host/ISO path M0–M3 documentado; boot stick físico pendente de evidência.

**QEMU lab ≠ metal:** `scripts/run-iso-qemu-wsl.sh` / `run-iso-qemu-direct.sh` → claim `Ring3_Linux/qemu` OBSERVED. Nunca promover a `VERIFIED` metal só com log QEMU.
