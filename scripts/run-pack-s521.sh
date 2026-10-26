#!/usr/bin/env bash
set -euo pipefail
# Usage: ./run-pack-s521.sh /nix/store/...-linux.iso/iso/*.iso
ISO="${1:?iso path}"
OUT=/mnt/c/Users/msrov/Downloads/nixaios-gnome-s521-transmute-persist.img
ROOT=~/src/nix-aios
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#dosfstools --out-link /tmp/nos-dosfstools
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:/usr/sbin:/usr/bin:$PATH"
bash "$ROOT/scripts/pack-usb-logfat.sh" "$ISO" "$OUT"
ls -lh "$OUT"
sha256sum "$OUT" | tee /tmp/nixaios-s521.sha256
