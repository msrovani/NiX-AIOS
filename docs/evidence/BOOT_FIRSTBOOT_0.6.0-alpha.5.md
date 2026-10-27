# Evidence — Boot→firstboot→JARBAS (0.6.0-alpha.5)

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux/qemu` — **OBSERVED** (não metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/r1bv6ai4xp5m243xjg76gs63i9g73vy5-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**SHA-256:** `13d2a7bc096f1fecc70749bef06db07a12adf81b8f2c637c4116ba857465df77`  
**Screenshot:** [`BOOT_FIRSTBOOT_0.6.0-alpha.5.png`](BOOT_FIRSTBOOT_0.6.0-alpha.5.png)

## Procedimento

Um único arranque, **sem comandos no guest** até ao fim:

```bash
ISO=<iso alpha.5> LOG=/tmp/a5.log DISPLAY_MODE=serial MEM=4G SMP=2 \
  QEMU_EXTRA="-device virtio-gpu-pci -vga none -monitor unix:/tmp/mon,server,nowait" \
  bash scripts/run-iso-qemu-wsl.sh
# (só no fim: systemctl poweroff)
```

`firstbootAuto = true` na ISO → o `nixos-aios-shell` corre o questionário em
modo auto quando o auto-login cai em tty1/ttyS0.

## Marcadores (guest)

```
modo=auto net_wait=8s
1) Rede (cabo ou Wi-Fi) ... Ja ligado: ens2 (ethernet) src=10.0.2.15
2) Idioma ... LANG=pt_BR.UTF-8
transmute RPC OK
  note=Downloads OK; hotswap disparado (log /var/lib/nosd/hotswap.log); models=/var/lib/nosd/models.
=== nosd-hotswap-aios DONE (jarbas-cage active, VT7) ===
=== nos-firstboot DONE (GUI up) ===
firstboot.done + jarbas-cage active
```

`/var/lib/nosd/hotswap.log`:

```
hotswap: wrote /etc/nixos/ai-managed.nix (locale=pt_BR.UTF-8)
hotswap: restarting nosd…            <-- (alpha.4; agora só se offline)
hotswap: nosd health OK
hotswap: starting jarbas-cage.service…
=== nosd-hotswap-aios DONE (jarbas-cage active, VT7) ===
```

Gate MQEMU:

```
stage=MQEMU ... layer=DELEGATE claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ... layer=ADAPT    claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
gate_summary fails=0 stage=MQEMU
```

## O que prova

- O gatilho do firstboot cobre sessões **seriais** (`ttyS0`) e o auto-login.
- Em modo auto não há prompts, não pendura o boot e **não descarrega** blobs.
- `transmute` responde OK (o `systemctl` sem root já não é contado como erro;
  o pedido é servido pelo `.path`), e o `hotswap` não mata o `nosd` que serve
  o RPC.
- `cage` arranca em DRM/KMS e o **HUD Slint aparece sem `systemctl restart`**
  (`chvt` no `ExecStartPre` root antes do `ExecStart`).
- O screenshot do scanout foi tirado ~8 min após o arranque, com a cadeia
  já concluída.

## O que **não** prova

- Metal: iGPU/dGPU, GL/femtovg, VT/teclado físicos, DPMS/VRR.
- Qualidade/performance do HUD (QEMU/TCG + `winit-software` + pixman).
- Download real de MoE/Whisper/Piper (auto usa `--no-download`).
- Wi-Fi (o QEMU usa ethernet slirp), áudio, webcam, M4/M5.

## Notas

- O artefacto de pixels no canto superior esquerdo que aparecia em alpha.4
  não ocorreu neste arranque (mesma ISO base + `chvt` antes do cage).
- `NOSD_FIRSTBOOT_ASK=1` repõe o questionário interactivo; útil em metal.
