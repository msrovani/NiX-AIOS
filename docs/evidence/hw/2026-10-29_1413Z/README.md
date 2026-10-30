# HW run — 2026-10-29T14:13Z (QEMU, img logfat)

Logs copiados da FAT `NIXAIOSLOG` (partição 3 do `.img`), sem segredos.

## Identificação (de `hw-info.log`)

- `Linux nixaios 6.18.57` · `console=tty0 console=ttyS0,115200n8`
- **QEMU**: `440FX` + `PIIX3`, VGA `1234:1111` (`bochs-drm`), NIC `82540EM`,
  `ens3` com `10.0.2.15`
- FAT: `/dev/sda3` montada (`fsck.fat: 2 files`), `GUEST_MOUNTED.txt` escrito

## O que correu

| Marco | Evidência |
|---|---|
| log-mount | `nixaios-log: OK /dev/sda3 -> /mnt/nixaios-log` |
| nosd | `Started Neural-OS Daemon`; `landlock Verified`; `memory backend=Nsgdb`; CRDT `seg.bin` replay=0 |
| mesh | `udp ON bind=0.0.0.0:19000`; HB com erro `Network is unreachable` (QEMU user net, esperado) |
| HUD | **`nos-jarbas` a logar `hud score=` a cada 2 s** (cage/DRM OK) |
| export | `journal-boot.log`, `journal-follow.log`, `dmesg-boot.log`, `hw-info.log` |

## O que **não** correu

| Sintoma | Causa |
|---|---|
| `engine=cpu-echo`, `hub/0` | `models/` vazio (`model-seed.log: total 0`) — sem GGUF na FAT |
| firstboot não terminou | parou no **Q2 (idioma)** à espera de input no tty1 (log com 16 linhas) |
| GUI "invisível" | com `-nographic` a HUD fica no DRM/tty1; precisa de `DISPLAY_MODE=vnc` (ou metal) |
| sem mic/som | ISO anterior ao fix do grupo `audio` (`e5bfa7c`) |
| câmara | `uiMode=slint` não tem pipeline; só `cinematic` (Chromium) |

## Conclusão

O caminho log-fat32 funciona (mount + export + seed). O bloqueio é **entrada
humana no Q2** e **falta de modelo**; a HUD já estava viva. Para uma corrida sem
interacção: `firstboot.conf` com `DONE=1` + `wifi.conf` + `models/*.gguf` na FAT.
