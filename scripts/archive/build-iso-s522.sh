#!/usr/bin/env bash
# ISO s522: firstboot TEXTO tty1 + transmute + Hotswap JARBAS (sem GNOME)
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG="/tmp/nixaios-iso-s522.out"
DEST_ISO="/mnt/c/Users/msrov/Downloads/nixaios-text-s522-transmute.iso"
DEST_IMG="/mnt/c/Users/msrov/Downloads/nixaios-text-s522-transmute-persist.img"

sed -i 's/\r$//' "$ROOT/scripts/build-iso-tmp.sh" "$ROOT/scripts/pack-usb-logfat.sh" "$ROOT/scripts/run-pack-s522.sh" || true
chmod +x "$ROOT/scripts/pack-usb-logfat.sh" "$ROOT/scripts/run-pack-s522.sh" "$ROOT/scripts/build-iso-tmp.sh"

export LOG="/tmp/nixaios-iso-build-s522.log"
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee "$LOG"
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
if [[ -z "$ISO" || ! -f "$ISO" ]]; then
  echo "NO_ISO out=$OUT" >&2
  exit 1
fi
cp -f "$ISO" "$DEST_ISO"
sha256sum "$DEST_ISO" | tee /tmp/nixaios-s522-iso.sha256

bash "$ROOT/scripts/run-pack-s522.sh" "$DEST_ISO"
sha256sum "$DEST_IMG" | tee /tmp/nixaios-s522-img.sha256
echo "COPIED_ISO=$DEST_ISO"
echo "COPIED_IMG=$DEST_IMG"
echo "VOLUME=NIXAIOS522 LOG_LABEL=NIXAIOSLOG"
