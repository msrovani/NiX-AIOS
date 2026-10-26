#!/usr/bin/env bash
# ISO s517: cortex nix ring + HF plan + wifi + log FAT .img
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG="/tmp/nixaios-iso-s517.out"
DEST_ISO="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s517.iso"
DEST_IMG="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s517-logfat.img"

sed -i 's/\r$//' "$ROOT/scripts/build-iso-tmp.sh" "$ROOT/scripts/pack-usb-logfat.sh" || true
chmod +x "$ROOT/scripts/pack-usb-logfat.sh"

bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee "$LOG"
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
if [[ -z "$ISO" || ! -f "$ISO" ]]; then
  echo "NO_ISO out=$OUT" >&2
  exit 1
fi
cp -f "$ISO" "$DEST_ISO"
sha256sum "$DEST_ISO" | tee /tmp/nixaios-s517-iso.sha256

if ! command -v mkfs.vfat >/dev/null || ! command -v sfdisk >/dev/null; then
  nix-shell -p dosfstools mtools util-linux --run \
    "bash '$ROOT/scripts/pack-usb-logfat.sh' '$DEST_ISO' '$DEST_IMG'"
else
  bash "$ROOT/scripts/pack-usb-logfat.sh" "$DEST_ISO" "$DEST_IMG"
fi
sha256sum "$DEST_IMG" | tee /tmp/nixaios-s517-img.sha256
echo "COPIED_ISO=$DEST_ISO"
echo "COPIED_IMG=$DEST_IMG"
echo "VOLUME=NIXAIOS517 LOG_LABEL=NIXAIOSLOG"
echo "HINT=Rufus DD no .img — nosctl plan | wifi-scan | nix-propose"
