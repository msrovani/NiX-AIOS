#!/usr/bin/env bash
set -euo pipefail
IMG="${1:-/mnt/c/Users/msrov/Downloads/nixaios-gnome-s520-transmute-persist.img}"
echo "IMG=$IMG"
ls -lh "$IMG"
sfdisk -d "$IMG"
# sfdisk -d: "... start=     6594560, size=     8388608, type=c"
START_SECT=$(sfdisk -d "$IMG" | sed -n 's/.*start=[[:space:]]*\([0-9][0-9]*\).*type=c.*/\1/p' | head -1)
echo "START_SECT=$START_SECT"
if [ -z "${START_SECT:-}" ]; then
  echo "FAIL: no FAT type=c partition"
  exit 1
fi
# Extract FAT into /tmp then mount loop (no mtools/nix required)
FAT=/tmp/nixaios-s520-fat.img
MNT=/tmp/nixaioslog-mnt
dd if="$IMG" of="$FAT" bs=512 skip="$START_SECT" count=8388608 status=progress
mkdir -p "$MNT"
umount "$MNT" 2>/dev/null || true
mount -o loop,ro,uid=0,gid=0 "$FAT" "$MNT"
echo "=== ls -la $MNT ==="
ls -la "$MNT" || true
echo "=== looking for runtime logs ==="
for f in journal-boot.log journal-follow.log transmute-launch.log wizard-watch.log hotswap.log jarbas-cage.log README.txt; do
  if [ -f "$MNT/$f" ]; then
    echo "---- $f ($(wc -c < "$MNT/$f") bytes) ----"
    head -c 8000 "$MNT/$f" || true
    echo
  else
    echo "MISS $f"
  fi
done
echo "=== nosd-persist ==="
ls -la "$MNT/nosd-persist" 2>/dev/null || echo "MISS nosd-persist/"
umount "$MNT" 2>/dev/null || true
