#!/usr/bin/env bash
# ISO 0.5.1: Linux-first persist + docs — volid NIXAIOS051
# POLICY: sem blobs de modelo/firmware na ISO (runtime fetch deliberado).
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_ISO="/mnt/c/Users/msrov/Downloads/nixaios-minimal-051.iso"
DEST_IMG="/mnt/c/Users/msrov/Downloads/nixaios-minimal-051-persist.img"
sed -i 's/\r$//' "$ROOT"/scripts/*.sh "$ROOT"/images/iso.nix "$ROOT"/modules/*.nix \
  "$ROOT"/packages/nosd/crates/nos-jarbas/ui/hud.slint \
  "$ROOT"/packages/nosd/crates/nos-jarbas/src/slint_app.rs 2>/dev/null || true
chmod +x "$ROOT/scripts/pack-usb-logfat.sh" "$ROOT/scripts/build-iso-tmp.sh" \
  "$ROOT/scripts/guard-iso-no-s1-blob.sh" "$ROOT/scripts/build-iso-050.sh"
export LOG="/tmp/nixaios-iso-build-051.log"
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee /tmp/nixaios-iso-051.out
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
[[ -n "$ISO" && -f "$ISO" ]] || { echo "NO_ISO out=$OUT"; exit 1; }
cp -f "$ISO" "$DEST_ISO"
bash "$ROOT/scripts/guard-iso-no-s1-blob.sh" "$DEST_ISO" || {
  echo "S1.2 REJECT guard failed — abort copy"
  exit 1
}
sha256sum "$DEST_ISO" | tee /tmp/nixaios-051-iso.sha256
# DD sem FAT32: Rufus grava a ISO pura; persistência = /var/lib/nosd.
cp -f "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_IMG" | tee /tmp/nixaios-051-img.sha256
python3 -c "from pathlib import Path;p=Path(r'$DEST_IMG');f=p.open('rb');f.seek(32768);print('pvd',f.read(2048)[40:72].decode('ascii','replace').strip())"
echo "COPIED_ISO=$DEST_ISO"
echo "COPIED_IMG=$DEST_IMG VOLUME=NIXAIOS051 UI=slint remig=0.5.1 NO_FAT32=1"
