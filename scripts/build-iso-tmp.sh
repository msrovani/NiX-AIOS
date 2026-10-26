#!/usr/bin/env bash
# Build ISO from a clean /tmp flake (avoids broken Windows→WSL .git clone)
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="${SRC:-$(cd "$SCRIPT_DIR/.." && pwd)}"
# Fallback Linux clone / Windows mount
if [[ ! -f "$SRC/flake.nix" ]]; then
  if [[ -f "$HOME/src/nix-aios/flake.nix" ]]; then
    SRC="$HOME/src/nix-aios"
  elif [[ -f /mnt/c/DEV/nix-aios/flake.nix ]]; then
    SRC="/mnt/c/DEV/nix-aios"
  fi
fi
WORK="${WORK:-/tmp/nixaios-flake}"
LOG="${LOG:-/tmp/nixaios-iso-build.log}"

echo "==> Sync $SRC -> $WORK"
rm -rf "$WORK"
mkdir -p "$WORK"
rsync -a --exclude target --exclude target1 --exclude .git "$SRC/" "$WORK/"
# Windows checkout often has CRLF; bash phases in nix break on $'\r'
find "$WORK" -type f \( -name '*.nix' -o -name '*.sh' -o -name '*.toml' -o -name '*.rs' \) \
  -exec sed -i 's/\r$//' {} +
cd "$WORK"
git init -q
git add -A
git -c user.email=nixaios@local -c user.name=nixaios commit -qm "nixaios iso build"

echo "==> nix build .#iso"
nix build .#iso -L --print-out-paths 2>&1 | tee "$LOG"
echo "BUILD_EXIT=${PIPESTATUS[0]}"
OUT=$(readlink -f result || true)
echo "RESULT=$OUT"
find "$OUT" -name '*.iso' 2>/dev/null || true
