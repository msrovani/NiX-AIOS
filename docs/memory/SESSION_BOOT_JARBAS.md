# Sessão — Boot → JARBAS (IDEA #707)

**Data:** 2026-10-26 · workspace 0.5.2+ · volid alvo `NIXAIOS051`.

## Entregue

- Playbook [`BOOT_TO_JARBAS.md`](../BOOT_TO_JARBAS.md) + gate `scripts/gate-boot-to-jarbas.sh`.
- M3 honesty: `route.explain` trinity/`Decision`/S1 LOO; `VoiceStatus.pipeline_stages`; `ui.card` HITL thin (0058).
- Campanhas [`METAL_CAMPAIGN.md`](../METAL_CAMPAIGN.md) / [`FLEET_LAB.md`](../FLEET_LAB.md) — M4/M5 **AWAITING_HW**.
- Gate stage `MQEMU` — QEMU ≠ metal.

## Aprendizado (anti-padrão)

1. UI todo completed ≠ aceite metal/Jarvis.
2. `cpu-echo` / `EngineAbsent` = OK honesty, não OK capability.
3. QEMU serial markers ≠ stick `NIXAIOS051` VERIFIED.
4. AgentJev sem blob → ScorerAbsent + LOO `blob_deliberate` (nunca seed ISO).

## QEMU lab (2026-10-26)

- ISO: `nixaios-minimal-s529-remig.iso` · TCG (sem KVM user) · `scripts/_qemu-boot-capture.sh` kernel+initrd.
- **OBSERVED:** systemd → Network → Serial Getty → autologin `nixos` + banner Neural AIOS.
- **FAILED:** `nosd.service` (Neural-OS Daemon) várias vezes no boot — claim QEMU ≠ metal VERIFIED; investigar journal guest.
- Gate: `NIXAIOS_QEMU_LOG=/tmp/nixaios-qemu.log bash scripts/gate-boot-to-jarbas.sh --stage MQEMU`.

## Próximo

Metal journal M4 → cage+chat live → lab 2-nó 0081 A/B · diagnosticar `nosd` FAIL no QEMU.