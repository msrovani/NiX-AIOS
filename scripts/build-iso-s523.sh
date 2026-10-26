#!/usr/bin/env bash
# ISO s523: firstboot texto — Q1 rede → idioma/voz → teclado → fuso → transmute/hotswap
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG="/tmp/nixaios-iso-s523.out"
DEST_ISO="/mnt/c/Users/msrov/Downloads/nixaios-text-s523-transmute.iso"
DEST_IMG="/mnt/c/Users/msrov/Downloads/nixaios-text-s523-transmute-persist.img"

sed -i 's/\r$//' "$ROOT/scripts/build-iso-tmp.sh" "$ROOT/scripts/pack-usb-logfat.sh" || true
chmod +x "$ROOT/scripts/pack-usb-logfat.sh" "$ROOT/scripts/build-iso-tmp.sh"

export LOG="/tmp/nixaios-iso-build-s523.log"
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee "$LOG"
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
if [[ -z "$ISO" || ! -f "$ISO" ]]; then
  echo "NO_ISO out=$OUT" >&2
  exit 1
fi
cp -f "$ISO" "$DEST_ISO"
sha256sum "$DEST_ISO" | tee /tmp/nixaios-s523-iso.sha256

nix build --extra-experimental-features 'nix-command flakes' nixpkgs#dosfstools --out-link /tmp/nos-dosfstools
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:/usr/sbin:/usr/bin:$PATH"
bash "$ROOT/scripts/pack-usb-logfat.sh" "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_IMG" | tee /tmp/nixaios-s523-img.sha256
python3 - <<'PY'
from pathlib import Path
p = Path("/mnt/c/Users/msrov/Downloads/nixaios-text-s523-transmute-persist.img")
with p.open("rb") as f:
    f.seek(32768)
    vol = f.read(2048)[40:72].decode("ascii", errors="replace").strip()
print("pvd_vol=", repr(vol), "size_GiB=", round(p.stat().st_size/1024**3, 2))
PY
echo "COPIED_ISO=$DEST_ISO"
echo "COPIED_IMG=$DEST_IMG"
echo "VOLUME=NIXAIOS523"
