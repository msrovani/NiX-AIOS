#!/usr/bin/env bash
set -euo pipefail
python3 <<'PY'
from pathlib import Path
iso = Path("/mnt/c/Users/msrov/Downloads/nixaios-text-s522-transmute.iso")
img = Path("/mnt/c/Users/msrov/Downloads/nixaios-text-s522-transmute-persist.img")
for p in (iso, img):
    with p.open("rb") as f:
        f.seek(32768)
        pvd = f.read(2048)
    vol = pvd[40:72].decode("ascii", errors="replace").strip()
    print(p.name, "size_GiB", round(p.stat().st_size / 1024**3, 2), "pvd_vol=", repr(vol))
PY

# nos-firstboot in built system (from last flake build workdir)
WORK=/tmp/nixaios-flake
if [ -L "$WORK/result" ]; then
  echo "result=$(readlink -f "$WORK/result")"
fi
# Find sw/bin/nos-firstboot via nix-store query of the system drv referenced by the iso build log
FOUND=$(find /nix/store -maxdepth 3 -path '*/sw/bin/nos-firstboot' 2>/dev/null | head -3 || true)
echo "nos-firstboot paths:"
echo "$FOUND"
if [ -n "$FOUND" ]; then
  FIRST=$(echo "$FOUND" | head -1)
  grep -E 'Q1|Idioma|nos-firstboot|jarbas-cage|hotswap' "$FIRST" | head -10 || true
fi
