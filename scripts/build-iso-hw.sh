#!/usr/bin/env bash
# Build da iso-hw + .img de pendrive único (ISO + FAT32 NIXAIOSLOG).
# Uso: bash scripts/build-iso-hw.sh
# Saída: Downloads/nixaios-hw-051.iso e ...-logfat.img (gravar o .img em DD).
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_ISO="${DEST_ISO:-/mnt/c/Users/msrov/Downloads/nixaios-hw-051.iso}"
DEST_IMG="${DEST_IMG:-/mnt/c/Users/msrov/Downloads/nixaios-hw-051-logfat.img}"
cd "$ROOT"

nix build .#iso-hw -o /tmp/iso-hw-result --print-build-logs
ISO=$(find -L /tmp/iso-hw-result -name '*.iso' -type f | head -1)
[ -n "$ISO" ] && [ -f "$ISO" ] || { echo "ERRO: ISO nao encontrada" >&2; exit 1; }
cp -f "$ISO" "$DEST_ISO"
sha256sum "$DEST_ISO"

nix shell nixpkgs#dosfstools nixpkgs#util-linux nixpkgs#mtools -c \
  bash "$ROOT/scripts/pack-hw-usb.sh" "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_IMG"

echo "ISO=$DEST_ISO"
echo "IMG=$DEST_IMG (gravar em DD: boot + NIXAIOSLOG no mesmo pendrive)"
