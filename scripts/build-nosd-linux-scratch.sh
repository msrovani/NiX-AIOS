#!/usr/bin/env bash
# Compila nosd para x86_64-linux via Nix (from scratch). Não usar cargo no Windows.
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Prefer Windows mount path if invoked from WSL with that layout
if [[ -d ~/src/nix-aios ]]; then
  SRC=~/src/nix-aios
else
  SRC="$ROOT"
fi
WORK=/tmp/nixaios-flake
LOG=/tmp/nixaios-nosd-linux-scratch.log

python3 "$SRC/scripts/fix-networkmanager-typo.py" || true

echo "==> Sync $SRC -> $WORK"
rm -rf "$WORK"
mkdir -p "$WORK"
rsync -a --exclude target --exclude target1 --exclude .git "$SRC/" "$WORK/"
find "$WORK" -type f \( -name '*.nix' -o -name '*.sh' -o -name '*.toml' -o -name '*.rs' \) \
  -exec sed -i 's/\r$//' {} +

if [[ ! -f "$WORK/packages/nosd/crates/nos-observe/Cargo.toml" ]]; then
  echo "FAIL: nos-observe missing after sync" >&2
  exit 1
fi
echo "NOS_OBSERVE_OK"

cd "$WORK"
git init -q
git add -A
git -c user.email=nixaios@local -c user.name=nixaios commit -qm "nixaios linux scratch"

echo "==> nix build .#nosd --rebuild (linux release)"
nix build .#nosd -L --rebuild --print-out-paths 2>&1 | tee "$LOG"
echo "NOSD_BUILD_EXIT=${PIPESTATUS[0]}"
OUT=$(readlink -f result)
echo "NOSD_OUT=$OUT"
file "$OUT/bin/nosd" "$OUT/bin/nosctl"
ls -lh "$OUT/bin/"
# ELF check
readelf -h "$OUT/bin/nosd" | grep -E 'Class|Machine|OS/ABI' || true
