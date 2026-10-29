#!/usr/bin/env bash
# ISO híbrida + partição FAT32 RW LABEL=NIXAIOSLOG no MESMO .img.
# Um só pendrive: boot + logs + firstboot.conf/wifi.conf + models/.
# Uso: pack-hw-usb.sh <iso> <out.img>
# Escrever o .img em DD (Rufus DD / dd); NÃO gravar o .iso em modo ISO.
set -euo pipefail

ISO="${1:?uso: pack-hw-usb.sh <iso> <out.img>}"
OUT="${2:?uso: pack-hw-usb.sh <iso> <out.img>}"
FAT_MB="${FAT_MB:-4096}"
LABEL="${LABEL:-NIXAIOSLOG}"

[ -f "$ISO" ] || { echo "ERRO: ISO ausente: $ISO" >&2; exit 1; }
for c in mkfs.vfat sfdisk dd truncate stat cp; do
  command -v "$c" >/dev/null || { echo "ERRO: falta $c no PATH" >&2; exit 1; }
done

ISO_SIZE=$(stat -c%s "$ISO")
ALIGN=$(( (ISO_SIZE + 1048575) / 1048576 * 1048576 ))
FAT_BYTES=$((FAT_MB * 1024 * 1024))
TOTAL=$((ALIGN + FAT_BYTES))
START_MB=$((ALIGN / 1048576))
START_SECT=$((ALIGN / 512))
SIZE_SECT=$((FAT_BYTES / 512))

echo "ISO=$ISO ($ISO_SIZE bytes)"
echo "FAT=${FAT_MB}MiB START_MB=$START_MB TOTAL=$TOTAL OUT=$OUT"

WORK_IMG="/tmp/nixaios-hw-usb.img"
FAT_TMP="/tmp/nixaios-hw-fat.img"
README="/tmp/nixaios-hw-readme.txt"
trap 'rm -f "$FAT_TMP" "$README" "$WORK_IMG"' EXIT

rm -f "$FAT_TMP" "$WORK_IMG"
truncate -s "$FAT_BYTES" "$FAT_TMP"
mkfs.vfat -F 32 -n "$LABEL" "$FAT_TMP"

cat >"$README" <<EOF
NIXAIOSLOG — mesmo pendrive da imagem NIXAIOS
=============================================
Esta partição FAT32 é escrita pelo guest e legível no Windows.

Modelo offline (opcional):
  models/active-qwen3.5-0.8b-q4_k_m.gguf

Wi-Fi (opcional; o Q1 também grava aqui):
  wifi.conf
    SSID=a-tua-rede
    PSK=a-tua-password

Respostas do questionário (gravadas pelo guest):
  firstboot.conf   DONE/LANG/KEYMAP/TZ

Logs (guest):
  journal-boot.log, journal-follow.log, firstboot.log, hotswap.log,
  jarbas-cage.log, wifi-diag.log, model-seed.log, dmesg-boot.log, hw-info.log
EOF
if command -v mcopy >/dev/null 2>&1; then
  mcopy -o -i "$FAT_TMP" "$README" ::/README.txt || true
fi

cp -f "$ISO" "$WORK_IMG"
chmod u+w "$WORK_IMG"
truncate -s "$TOTAL" "$WORK_IMG"
printf 'start=%s,size=%s,type=c\n' "$START_SECT" "$SIZE_SECT" \
  | sfdisk --append --no-reread --force "$WORK_IMG"
dd if="$FAT_TMP" of="$WORK_IMG" bs=1M seek="$START_MB" conv=notrunc status=none

cp -f "$WORK_IMG" "$OUT"
echo "OK $OUT"
sfdisk -l "$OUT" 2>/dev/null | tail -6 || true
ls -lh "$OUT"
