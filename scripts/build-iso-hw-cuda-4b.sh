#!/usr/bin/env bash
# Build iso-hw-cuda-4b + .img (ISO + FAT32 NIXAIOSLOG).
# Só Qwen3.5-4B-Q6_K. Uso: bash scripts/build-iso-hw-cuda-4b.sh
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_ISO="${DEST_ISO:-/mnt/c/Users/msrov/Downloads/nixaios-hw-cuda-4b.iso}"
DEST_IMG="${DEST_IMG:-/mnt/c/Users/msrov/Downloads/nixaios-hw-cuda-4b-logfat.img}"
cd "$ROOT"

# flake usa a árvore git — ficheiros novos têm de estar staged.
git add -f images/hw-4b.nix images/hw-cuda-4b.nix scripts/build-iso-hw-cuda-4b.sh \
  packages/nosd/crates/nos-cortex/src/slot_map.rs flake.nix 2>/dev/null || true

NIXPKGS_ALLOW_UNFREE=1 nix build --impure .#iso-hw-cuda-4b -o /tmp/iso-hw-cuda-4b-result --print-build-logs
ISO=$(find -L /tmp/iso-hw-cuda-4b-result -name '*.iso' -type f | head -1)
[ -n "$ISO" ] && [ -f "$ISO" ] || { echo "ERRO: ISO nao encontrada" >&2; exit 1; }
cp -f "$ISO" "$DEST_ISO"
sha256sum "$DEST_ISO"

# FAT menor: só um GGUF na ISO; seed opcional no stick.
FAT_MB="${FAT_MB:-2048}"
export FAT_MB
nix shell nixpkgs#dosfstools nixpkgs#util-linux nixpkgs#mtools -c \
  bash "$ROOT/scripts/pack-hw-usb.sh" "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_IMG"

echo "ISO=$DEST_ISO"
echo "IMG=$DEST_IMG (gravar em DD: boot + NIXAIOSLOG)"
echo "Modelo: Qwen3.5-4B-Q6_K apenas (active.gguf)"
