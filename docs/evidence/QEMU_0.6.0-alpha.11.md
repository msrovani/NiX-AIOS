# Evidence — ISO/QEMU 0.6.0-alpha.11

**Data:** 2026-10-27
**Ring:** `Ring3_Linux/qemu` — OBSERVED (não metal)
**Volid:** `NIXAIOS051`
**ISO:** `/nix/store/49ym2x1sbh52vkddqdkxjpgmj7wwfi7k-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`
**SHA-256:** `4a6f2957785f8802f910304a011bc692de878a8afad77f0a6c5b5074bd0fb643`
**Cópia:** `/mnt/c/Users/msrov/Downloads/nixaios-minimal-051.iso`
**Closure:** contém `nosd-0.6.0-alpha.11` (verificado dentro do guest: `ls -d /nix/store/*-nosd-0.6.0-alpha.11`)

## Comando

```bash
ISO=<path> LOG=docs/evidence/artifacts/qemu-0.6.0-alpha.11.serial.log \
  DISPLAY_MODE=serial MEM=4G SMP=2 timeout 240 \
  bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=docs/evidence/artifacts/qemu-0.6.0-alpha.11.serial.log \
  bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

Execução em **TCG** (sem `/dev/kvm` acessível no WSL nesta sessão).

## Resultado

```
stage=MQEMU ring=Ring3_Linux_qemu layer=DELEGATE claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ring=Ring3_Linux_qemu layer=ADAPT claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
---
gate_summary fails=0 stage=MQEMU idea=#707 playbook=docs/BOOT_TO_JARBAS.md
```

Artefactos: `qemu-0.6.0-alpha.11.serial.log`, `.gate.txt` (pasta `artifacts/`).

## Marcadores observados no serial

- `Reached target` / systemd a subir;
- `Started nosd-hotswap-aios.path`, `nosd-nix-apply.path`, `nosd-nix-rollback.path`;
- `Auto-enter JARBAS if /var/lib/nosd already transmuted` (finished);
- autologin `nixos` e shell de bootstrap (`[aios] tty=/dev/ttyS0`);
- firstboot a correr em serviço (`nos-firstboot ja em curso — a sair`).

Dentro dos 240 s **não** se observou `nos-firstboot DONE` nem
`jarbas-cage ACTIVE` nesta execução (TCG, 240 s). O gate valida o arranque e o
`nosd`, não a GUI. A GUI/HUD foi observada noutra execução (ver release,
“Demo manual 3-VM”).

## Observações (honestidade)

- `[FAILED] Failed to start Virtual Console Setup.` aparece **3×** (também na
  alpha.10). Não bloqueia o boot nem o `nosd`; continua por investigar.
- O firstboot (offline) falha o passo `transmute` por `--no-download` duplicado
  (`args cannot be used multiple times`) e cai para a GUI. Bug registado para a
  alpha.12 em `docs/RELEASE_0.6.0-alpha.11.md`.
- **QEMU ≠ metal** (M4 continua `AWAITING_HW`); boot da ISO não é a frota.
- Sem modelo na ISO (política S1.2); sem downloads no firstboot por omissão.
- A ISO foi construída a partir da árvore de release antes dos documentos
  finais; os docs não entram na imagem.
