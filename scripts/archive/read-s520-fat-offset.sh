#!/usr/bin/env bash
set -euo pipefail
IMG=/mnt/c/Users/msrov/Downloads/nixaios-gnome-s520-transmute-persist.img
MNT=/tmp/nixaioslog-mnt
OFF=$((6594560 * 512))
mkdir -p "$MNT"
umount "$MNT" 2>/dev/null || true
if ! mount -o loop,ro,offset=$OFF "$IMG" "$MNT"; then
  sudo mount -o loop,ro,offset=$OFF "$IMG" "$MNT"
fi
echo "=== ls $MNT ==="
ls -la "$MNT" || true
echo "=== runtime logs ==="
for f in journal-boot.log journal-follow.log transmute-launch.log wizard-watch.log hotswap.log jarbas-cage.log README.txt; do
  if [ -f "$MNT/$f" ]; then
    echo "---- FOUND $f ----"
    wc -c "$MNT/$f"
    head -n 60 "$MNT/$f" || true
    echo
  else
    echo "MISS $f"
  fi
done
echo "=== nosd-persist ==="
ls -laR "$MNT/nosd-persist" 2>/dev/null || echo "MISS nosd-persist/"
umount "$MNT" 2>/dev/null || sudo umount "$MNT" 2>/dev/null || true
