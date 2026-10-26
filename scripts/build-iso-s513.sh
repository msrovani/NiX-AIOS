#!/usr/bin/env bash
# ISO s513 — cinematic orb HUD; copy → Downloads (Rufus DD).
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG="/tmp/nixaios-iso-s513.out"
DEST="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s513-cinematic-orb.iso"

sed -i 's/\r$//' "$ROOT/scripts/build-iso-tmp.sh" || true
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee "$LOG"
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
if [[ -z "$ISO" || ! -f "$ISO" ]]; then
  echo "NO_ISO out=$OUT" >&2
  exit 1
fi
cp -f "$ISO" "$DEST"
ls -lh "$DEST"
sha256sum "$DEST" | tee /tmp/nixaios-s513.sha256
echo "COPIED=$DEST"
echo "VOLUME=NIXAIOS513"
