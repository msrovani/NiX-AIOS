#!/usr/bin/env bash
# NixAIOS — build bootable ISO via flake (run inside WSL Ubuntu with Nix)
# Output: result/iso/*.iso (symlink result/)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Prefer repo that contains this script; else Linux clone; else Windows mount.
if [[ -f "$SCRIPT_DIR/../flake.nix" ]]; then
  NIXAIOS="$(cd "$SCRIPT_DIR/.." && pwd)"
elif [[ -d "${REPO:-$HOME/src/nix-aios}" ]]; then
  NIXAIOS="${REPO:-$HOME/src/nix-aios}"
elif [[ -d /mnt/c/DEV/nix-aios ]]; then
  NIXAIOS="/mnt/c/DEV/nix-aios"
else
  echo "NixAIOS repo not found (set REPO=...)" >&2
  exit 1
fi

cd "$NIXAIOS"
echo "==> Building NixAIOS ISO from $NIXAIOS"
echo "    (first run downloads nixpkgs — can take 30–90+ min)"

if nix build .#iso -L --print-out-paths 2>/tmp/nixaios-iso-build.log; then
  OUT=$(readlink -f result)
else
  echo "    .#iso failed, trying nixosConfigurations path..."
  nix build .#nixosConfigurations.nixaios-iso.config.system.build.isoImage -L
  OUT=$(readlink -f result)
fi

echo ""
echo "ISO build OK → $OUT"
find "$OUT" -name '*.iso' -printf '  %p (%s bytes)\n' 2>/dev/null || find "$OUT" -name '*.iso'
echo ""
echo "Copy to Windows Downloads (optional):"
echo "  cp -v \$OUT/*.iso /mnt/c/Users/\$USER/Downloads/  # adjust USER"
echo "Honesty: WSL build PASS ≠ metal boot PASS (ADR-0113)."
