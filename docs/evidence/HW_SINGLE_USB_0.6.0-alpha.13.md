# Pendrive único (ISO + NIXAIOSLOG) — 0.6.0-alpha.13

**Data:** 2026-10-29
**Imagem:** `nixaios-hw-051-logfat.img` (ISO `iso-hw` + FAT32 4 GiB
`NIXAIOSLOG`, mesmo dispositivo).
**SHA-256 `.img`:** `c58bdb48885c514a6be381a1ff5e8296b8ea5f15d7a3b0928367dfde4161a354`
(7 178 551 296 bytes)
**SHA-256 ISO embutida:** `03b236d20fd3fa960607643271a851d42502dcd34ee250ebd33ce7194bbe4bce`
**Estado:** imagem + mount **OBSERVED em QEMU (BIOS)**; metal e persistência
`firstboot.conf` **AWAITING_HW**.

## O que foi validado

- `pack-hw-usb.sh` acrescenta uma partição `p3` tipo `c` (W95 FAT32 LBA) depois
  da ISO; MBR/El Torito BIOS+UEFI da ISO ficam intactos.
- `blkid` na partição: `LABEL=NIXAIOSLOG`, FAT32, 4 GiB.
- `README.txt` presente na FAT (`mdir`).
- Boot BIOS do `.img` em QEMU (TCG): chega ao getty/shell serial (~120 s).
- Guest enumera `sda3` como `vfat NIXAIOSLOG` (`lsblk`/`blkid`) e monta:
  `FAT_MOUNTED`; `/mnt/nixaios-log` tem `README.txt`, `GUEST_MOUNTED.txt`,
  `journal-boot.log`, `journal-follow.log`, `dmesg-boot.log`, `hw-info.log`,
  `firstboot.log` e `model-seed.log` (o guest escreveu na mesma stick).

Saída relevante (QEMU BIOS):

```
sda     6,7G iso9660  NIXAIOS051
├─sda1  2,7G iso9660  NIXAIOS051
├─sda2    3M vfat     EFIBOOT
└─sda3    4G vfat     NIXAIOSLOG
mountpoint -q /mnt/nixaios-log && echo FAT_MOUNTED
FAT_MOUNTED
```

## Bug encontrado (lição 2/24)

`path` estava **dentro de `serviceConfig`** nas units
`nixaios-log-mount`, `nixaios-log-export`, `nixaios-model-seed` e
`nixaios-wifi-seed`. O systemd renderizou `path=` como lvalue inválido em
`[Service]` e ignorou-o: sem `util-linux` no PATH, `mount`/`mountpoint` não
existiam e a FAT nunca montava (o export re-tentava mas também sem
`gawk`/`gnused`). Corrigido movendo `path` para o nível do unit; verificado no
artefacto (PATH com `util-linux`/`dosfstools`/`gawk`).

## Como reproduzir

```bash
bash scripts/build-iso-hw.sh          # ISO + .img (Downloads)
# gravar o .img em DD; um só pendrive
```
