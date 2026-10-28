# Evidence — ISO/QEMU 0.6.0-alpha.10

**Data:** 2026-10-27
**Ring:** `Ring3_Linux/qemu` — OBSERVED (não metal)
**Volid:** `NIXAIOS051`
**ISO:** `/nix/store/gm542dq0kbwzha7lc1s04hikhdxvrns7-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`
**SHA-256:** `98218b99ad3812e0ceef52b53e27e500644dd1df91df60bff7cf7aa09c32f115`
**Cópia:** `/mnt/c/Users/msrov/Downloads/nixaios-minimal-051.iso` (2 882 863 104 bytes)
**Closure:** contém `nosd-0.6.0-alpha.10.drv` (1 referência)

## Comando

```bash
ISO=<path> LOG=docs/evidence/artifacts/qemu-0.6.0-alpha.10.serial.log \
  DISPLAY_MODE=serial MEM=4G SMP=2 timeout 220 \
  bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=docs/evidence/artifacts/qemu-0.6.0-alpha.10.serial.log \
  bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

Esta execução correu em **TCG** (`KVM unavailable` — o utilizador não está no
grupo `kvm` nesta sessão WSL), não em KVM.

## Resultado

```
stage=MQEMU ring=Ring3_Linux_qemu layer=DELEGATE claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ring=Ring3_Linux_qemu layer=ADAPT claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
---
gate_summary fails=0 stage=MQEMU idea=#707 playbook=docs/BOOT_TO_JARBAS.md
```

Artefactos: `docs/evidence/artifacts/qemu-0.6.0-alpha.10.gate.txt` e o serial
completo `qemu-0.6.0-alpha.10.serial.log`.

### Marcadores no serial

| Marcador | Ocorrências |
|---|---|
| `Reached target` | 40 |
| `nosd health OK` | 1 |
| `HITL grant obtido` | 1 |
| `transmute RPC OK` | 1 |
| `jarbas-cage ACTIVE` | 1 |
| `nos-firstboot DONE` | 1 |

O arranque passou por: nosd healthy → grant HITL server-authored
(`kind=transmute`) → transmute `--no-download` → hotswap → `jarbas-cage`
activo → HUD Slint no tty7 (prime-boot automático).

## Observações (honestidade)

- `[FAILED] Failed to start Virtual Console Setup.` aparece **2×** no serial.
  Não bloqueia o firstboot/JARBAS (`DONE` + `jarbas-cage ACTIVE`), mas fica
  registado: não foi investigado nesta release e não é declarado “limpo”.
- `Linux version` não aparece neste serial (o log começa em SeaBIOS/console);
  o gate valida marcadores systemd + `nosd` start, não a linha do kernel.
- Não há prova do backend `NSGC` **dentro** desta ISO: o log do nosd vai para
  o journald, não para o serial, e o disco do QEMU é efémero. A prova do
  segmento está nos checks de frota (`mesh-2vm-crdt`, `NSGC` nos dois nós) e
  nos testes de daemon; o que a ISO prova é que **este build** (closure com
  `nosd-0.6.0-alpha.10`) arranca e sobe o JARBAS.
- **QEMU ≠ metal**: M4 continua `AWAITING_HW`.
- Boot da ISO **não** é a frota: os checks `mesh-2vm*` correm em VMs separadas.
- Sem modelo na ISO (política S1.2); o transmute corre `--no-download`, os
  blobs vêm por fetch/HITL em runtime.
- A ISO foi construída a partir da árvore de release (cópia sem `.git` em
  `/tmp/nixaios-flake`); os documentos de release foram escritos depois e não
  entram na imagem.
