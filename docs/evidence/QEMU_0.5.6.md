# Evidence — ISO 0.5.6 / QEMU (R-09)

**Data:** 2026-10-26  
**Ring:** `Ring3_Linux/qemu` — **OBSERVED** (não é metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/1ris0a5c59i51ky6x3vc7sp5kbj12h6g-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**Tamanho:** 2.7 GiB  
**SHA-256:** `26be2fb47a6e2a3b194dd9b5b1ef1a853d1450cc7b34a24dbafb00e5f09fc11b`  
**PVD volume ID:** `NIXAIOS051` (bytes `4e 49 58 41 49 4f 53 30 35 31`)

## Comando

```bash
ISO=<path> LOG=/tmp/nixaios-qemu-055b.log DISPLAY_MODE=serial MEM=4G SMP=2 \
  timeout -k 10 600 bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=/tmp/nixaios-qemu-055b.log \
  bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

TCG (sem `/dev/kvm`; user fora do grupo kvm), 4 GiB, 2 vCPU, console serial
`ttyS0` (adicionado em `images/iso.nix`).

## Marcadores observados

```
[  OK  ] Started Neural-OS Daemon.
[  OK  ] Started NixAIOS early boot: OBSERVE_SNAPSHOT + BOOT SCORE (pre-getty).
[  OK  ] Finished NixAIOS early boot: OBSERVE_SNAPSHOT + BOOT SCORE (pre-getty).
[  OK  ] Started JARBAS hub slog (sem DRM).
[  OK  ] Reached target Multi-User System.
nixaios login: nixos (início de sessão automático)
* Neural AIOS — shell de bootstrap *
```

Gate:

```
stage=MQEMU ... claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ... claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
gate_summary fails=0 stage=MQEMU
```

## VM test (R-05)

```bash
nix build .#checks.x86_64-linux.nixaios-vm -L
# test script finished in 61.42s
# nosctl health -> ok
# nosctl chat oi -> ok (cpu-echo)
```

O check `checks.x86_64-linux.nixaios-vm` sobe NixOS com `services.nosd`, espera
`nosd.service`, o socket Unix, e executa `nosctl health` + `nosctl chat oi`.

## O que isto prova / não prova

**Prova:** ISO constrói, kernel+initrd+switch-root, systemd multi-user, `nosd`
arranca, `nosd-early` completa, JARBAS hub sobe, serial login.

**Não prova:** chat GGUF, voz, cage/DRM, Wi-Fi, metal, 2-nós. `MQEMU` ≠
`Ring3_Linux/metal`; não promover M4/M5.

**Nota:** o firstboot de texto roda em tty1; no serial `ttyS0` o login shell
imprime o banner mas não dispara `nos-firstboot` (condição de tty). QEMU M2/M3b
precisa de interação em tty1 ou ajuste de condição para `ttyS0` em laboratório.
