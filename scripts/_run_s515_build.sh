#!/usr/bin/env bash
set -euo pipefail
ROOT=~/src/nix-aios/scripts
python3 - <<'PY'
from pathlib import Path
root = Path("~/src/nix-aios/scripts")
for name in ("build-iso-s515.sh", "build-iso-tmp.sh", "_run_s515_build.sh"):
    p = root / name
    if not p.exists():
        continue
    b = p.read_bytes().replace(b"\r\n", b"\n").replace(b"\r", b"\n")
    p.write_bytes(b)
    print("fixed", name)
PY
chmod +x "$ROOT/build-iso-s515.sh" "$ROOT/_run_s515_build.sh"
pkill -f build-iso-s515 || true
pkill -f 'nix build .#iso' || true
sleep 1
nohup bash "$ROOT/build-iso-s515.sh" >/tmp/nixaios-s515-nohup.out 2>&1 &
echo "PID=$!"
sleep 5
head -50 /tmp/nixaios-s515-nohup.out
