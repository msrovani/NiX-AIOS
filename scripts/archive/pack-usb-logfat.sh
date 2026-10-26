#!/usr/bin/env bash
# ISO híbrida + partição FAT32 RW LABEL=NIXAIOSLOG (logs + persistência modelos MoE/STT/TTS) → .img Rufus DD
# Uso: pack-usb-logfat.sh <iso> <out.img>
set -euo pipefail

ISO="${1:?iso path}"
OUT="${2:?out.img path}"
# 4096 MiB (4 GiB) cabe modelos MoE Q4 + Whisper + Piper + logs no pendrive de 30 GB
FAT_MB="${FAT_MB:-4096}"
LABEL="${LABEL:-NIXAIOSLOG}"

command -v mkfs.vfat >/dev/null
command -v sfdisk >/dev/null
command -v dd >/dev/null

ISO_SIZE=$(stat -c%s "$ISO")
ALIGN=$(( (ISO_SIZE + 1048575) / 1048576 * 1048576 ))
FAT_BYTES=$((FAT_MB * 1024 * 1024))
TOTAL=$((ALIGN + FAT_BYTES))
START_MB=$((ALIGN / 1048576))
START_SECT=$((ALIGN / 512))
SIZE_SECT=$((FAT_BYTES / 512))

echo "ISO=$ISO ($ISO_SIZE bytes)"
echo "ALIGN=$ALIGN FAT=${FAT_MB}MiB TOTAL=$TOTAL OUT=$OUT"

# Monta em /tmp (ext4 rápido do WSL) e só no fim copia p/ /mnt/c
WORK_IMG="/tmp/nixaios-pack-usb.img"
FAT_TMP="/tmp/nixaios-fat-part.img"
README="/tmp/nixaios-readme.txt"
trap 'rm -f "$FAT_TMP" "$README" "$WORK_IMG"' EXIT

rm -f "$FAT_TMP" "$WORK_IMG"
truncate -s "$FAT_BYTES" "$FAT_TMP"
mkfs.vfat -F 32 -n "$LABEL" "$FAT_TMP"

cat >"$README" <<EOF
NixAIOS Persistent & Log Partition (FAT32) — LABEL=$LABEL
=========================================================
1. Logs (readable in Windows):
   - journal-boot.log    snapshot at boot
   - journal-follow.log  live append
   - jarbas-cage.log     Wayland cage + Chromium orb
   - hotswap.log         nos-transmute GUI -> JARBAS hotswap log

2. Persistent AIOS Data (nosd-persist/):
   - models/             Downloaded MoE GGUF + Whisper STT + Piper TTS models
   - locale.conf         Selected system language (e.g. pt_BR.UTF-8)
   - aios-mode           Set to 'aios' after conversion
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
ls -lh "$OUT"
