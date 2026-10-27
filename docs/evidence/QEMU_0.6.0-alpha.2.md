# Evidence — ISO / QEMU 0.6.0-alpha.2

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux/qemu` — OBSERVED (não metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/mc8kxm0mywz445xkp4zr8jgnl36xj0dg-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**SHA-256:** `6f9f2906501bd8cca826297d7e9f7ee4a97ea373d7f4ec42f84c4ab9c2f0d812`  
**PVD:** `NIXAIOS051` (bytes `4e 49 58 41 49 4f 53 30 35 31`)

## Comando

```bash
ISO=<path> LOG=/tmp/nixaios-qemu-060a2.log DISPLAY_MODE=serial MEM=4G SMP=2 \
  timeout -k 10 600 bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=/tmp/nixaios-qemu-060a2.log \
  bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

## Marcadores

```
[  OK  ] Started Neural-OS Daemon.
[  OK  ] Started NixAIOS early boot: OBSERVE_SNAPSHOT + BOOT SCORE (pre-getty).
[  OK  ] Finished NixAIOS early boot: OBSERVE_SNAPSHOT + BOOT SCORE (pre-getty).
[  OK  ] Started JARBAS hub slog (sem DRM).
[  OK  ] Reached target Multi-User System.
nixaios login: nixos (início de sessão automático)
```

Gate:

```
stage=MQEMU ... claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ... claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
gate_summary fails=0 stage=MQEMU
```

## Notas

- Esta ISO inclui a correção R-01 real (`systemd.services.nosd.path`) e o
  Landlock RO (`/nix/store`, `/etc`, `/run`, ...); `nosd` arranca com
  `landlock Verified`.
- Sem modelo na ISO (política S1.2). O teste com GGUF/`llama-server` é o VM
  test (`VM_GGUF_0.6.0-alpha.2.md`), não este boot.
- QEMU ≠ metal; M4/M5 continuam AWAITING_HW.
