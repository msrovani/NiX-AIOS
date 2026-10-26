#!/usr/bin/env bash
set -euo pipefail
# Usage: ./run-pack-s522.sh /nix/store/...-linux.iso/iso/*.iso
ISO="${1:?iso path}"
OUT=/mnt/c/Users/msrov/Downloads/nixaios-text-s522-transmute-persist.img
ROOT=~/src/nix-aios
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#dosfstools --out-link /tmp/nos-dosfstools
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:/usr/sbin:/usr/bin:$PATH"
bash "$ROOT/scripts/pack-usb-logfat.sh" "$ISO" "$OUT"
ls -lh "$OUT"
sha256sum "$OUT" | tee /tmp/nixaios-s522.sha256
echo "volume check:"
python3 - <<'PY'
from pathlib import Path
p = Path("/mnt/c/Users/msrov/Downloads/nixaios-text-s522-transmute-persist.img")
b = p.read_bytes()[:512*64]
print("NIXAIOS522", "OK" if b"NIXAIOS522" in b else "MISS")
print("size_GiB", round(p.stat().st_size/1024**3, 2))
PY
