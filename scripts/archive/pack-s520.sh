#!/usr/bin/env bash
set -euo pipefail
STORE=/nix/store/5z9f6hl66wl4cryzkvgnfq0kcb9fblkz-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso
ISO="$STORE/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso"
OUT=/mnt/c/Users/msrov/Downloads/nixaios-gnome-s520-transmute-persist.img
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ls -lh "$ISO"
bash "$ROOT/scripts/pack-usb-logfat.sh" "$ISO" "$OUT"
ls -lh "$OUT"
sha256sum "$OUT" | tee /tmp/nixaios-s520.sha256
# volume ID probe (ISO9660 Primary Volume Descriptor at 32768)
python3 - <<'PY'
from pathlib import Path
p = Path("/mnt/c/Users/msrov/Downloads/nixaios-gnome-s520-transmute-persist.img")
b = p.read_bytes()[: 8 * 1024 * 1024]
print("NIXAIOS520", "OK" if b"NIXAIOS520" in b else "MISS")
# FAT label is in the second partition; search whole first 32MiB of img after align is heavy —
# pack script sets LABEL=NIXAIOSLOG via mkfs.vfat -n
print("size_gb", round(p.stat().st_size / (1024**3), 2))
PY
