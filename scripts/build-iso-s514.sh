#!/usr/bin/env bash
# ISO s514 — chvt + Slint software; copy → Downloads (Rufus DD).
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG="/tmp/nixaios-iso-s514.out"
DEST="/mnt/c/Users/msrov/Downloads/nixaios-minimal-s514-ui-visible.iso"

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
sha256sum "$DEST" | tee /tmp/nixaios-s514.sha256
echo "COPIED=$DEST"
echo "VOLUME=NIXAIOS514"
echo "HINT=se ainda shell: Alt+F7 — journalctl -u jarbas-cage -u jarbas-show-vt"
