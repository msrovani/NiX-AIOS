#!/usr/bin/env bash
set -euo pipefail
ISO=/nix/store/5z9f6hl66wl4cryzkvgnfq0kcb9fblkz-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso
OUT=/mnt/c/Users/msrov/Downloads/nixaios-gnome-s520-transmute-persist.img
ROOT=~/src/nix-aios

echo "fetching dosfstools+mtools…"
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#dosfstools --out-link /tmp/nos-dosfstools
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:/usr/sbin:/usr/bin:$PATH"
command -v mkfs.vfat
command -v sfdisk
command -v mcopy || echo "mcopy optional"

bash "$ROOT/scripts/pack-usb-logfat.sh" "$ISO" "$OUT"
ls -lh "$OUT"
sha256sum "$OUT" | tee /tmp/nixaios-s520.sha256
