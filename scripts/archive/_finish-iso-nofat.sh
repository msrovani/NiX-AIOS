#!/usr/bin/env bash
# Copy nix result ISO → Downloads; DD = same ISO (no FAT partition).
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true

DEST_ISO="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s529-remig.iso"
DEST_IMG="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s529-remig-persist.img"
ISO="/nix/store/0mkv2k5syy6c8s85w8zjwdyszksyg102-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso"

# Prefer result link if present (fresher rebuild)
if [[ -L /tmp/nixaios-flake/result ]]; then
  OUT=$(readlink -f /tmp/nixaios-flake/result || true)
  CAND=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
  [[ -n "$CAND" && -f "$CAND" ]] && ISO="$CAND"
fi

echo "ISO=$ISO"
[[ -f "$ISO" ]] || { echo "NO_ISO"; exit 1; }

echo "copying $(du -h "$ISO" | awk '{print $1}') …"
cp -f "$ISO" "$DEST_ISO"
if [[ -f ~/src/nix-aios/scripts/guard-iso-no-s1-blob.sh ]]; then
  bash ~/src/nix-aios/scripts/guard-iso-no-s1-blob.sh "$DEST_ISO" || {
    echo "S1.2 REJECT"; exit 1;
  }
fi
cp -f "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_ISO" | tee /tmp/nixaios-s529-iso.sha256
sha256sum "$DEST_IMG" | tee /tmp/nixaios-s529-img.sha256
python3 - <<'PY'
from pathlib import Path
p = Path("/mnt/c/Users/msrov/Downloads/nixaios-minimal-s529-remig-persist.img")
with p.open("rb") as f:
    f.seek(32768)
    print("pvd", f.read(2048)[40:72].decode("ascii", "replace").strip())
iso = Path("/mnt/c/Users/msrov/Downloads/nixaios-minimal-s529-remig.iso")
print("iso_bytes", iso.stat().st_size, "img_bytes", p.stat().st_size, "same", iso.stat().st_size == p.stat().st_size)
PY
echo "COPIED_ISO=$DEST_ISO"
echo "COPIED_IMG=$DEST_IMG VOLUME=NIXAIOS529 NO_FAT32=1"
ls -lh "$DEST_ISO" "$DEST_IMG"
