#!/usr/bin/env bash
# Launch ISO rebuild s510 (Wayland fail-closed) and copy to Downloads.
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG="/tmp/nixaios-iso-s510.out"
DEST="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s511-cage-noloop.iso"

sed -i 's/\r$//' "$ROOT/scripts/build-iso-tmp.sh" || true
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee "$LOG"
# build-iso-tmp tees to /tmp/nixaios-iso-build.log and prints BUILD_EXIT
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
if [[ -z "$ISO" || ! -f "$ISO" ]]; then
  echo "NO_ISO out=$OUT" >&2
  exit 1
fi
cp -f "$ISO" "$DEST"
ls -lh "$DEST"
sha256sum "$DEST" | tee /tmp/nixaios-s510.sha256
echo "COPIED=$DEST"
