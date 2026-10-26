#!/usr/bin/env bash
# Launcher: ISO s529 sem FAT (logs em /var/lib/nosd)
set -euo pipefail
exec > >(tee /tmp/nixaios-iso-s529-nofat.out) 2>&1
echo "=== start $(date -Is) pid=$$ ==="
bash ~/src/nix-aios/scripts/build-iso-s529.sh
echo "=== done $(date -Is) exit=$? ==="
