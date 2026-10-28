# Evidence ? ISO/QEMU 0.6.0-alpha.9

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux/qemu` ? OBSERVED (n?o metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/8afqch2qj18i8kycdvssc77jjn6pr3xl-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**SHA-256:** `ff1f3f53d675ce4639d32ddd26f6a39563bf30302c839c2aaf76ac20a1a86a1b`

## Comando

```bash
ISO=<path> LOG=/tmp/a9.log DISPLAY_MODE=serial MEM=4G SMP=2 \
  bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=/tmp/a9.log bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

## Resultado

```
stage=MQEMU ... layer=DELEGATE claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ... layer=ADAPT    claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
gate_summary fails=0 stage=MQEMU
```

`nosd` arranca com as novas pe?as: identidade opcional (sem chave = compat),
log CRDT em `/var/lib/nosd/crdt.jsonl` (sem identidade ? linhas simples),
listener de jobs desligado (`NOSD_JOB_BIND` n?o setado).

## Honestidade

- Boot validado ? frota validada: os checks `mesh-2vm*` correm em VMs
  separadas, n?o dentro desta ISO.
- QEMU ? metal (M4 continua AWAITING_HW).
- Sem modelo na ISO (pol?tica S1.2); GGUF/voz t?m evid?ncia VM pr?pria.
