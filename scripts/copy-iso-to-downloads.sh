#!/usr/bin/env bash
set -euo pipefail
# Prefer result symlink from last build; else explicit store path env ISO_STORE
if [[ -n "${ISO_STORE:-}" ]]; then
  ISO="${ISO_STORE}/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso"
elif [[ -L /tmp/nixaios-flake/result ]]; then
  ISO="$(readlink -f /tmp/nixaios-flake/result)/iso/"*.iso
  # expand glob
  ISO=$(ls "$(readlink -f /tmp/nixaios-flake/result)"/iso/*.iso | head -1)
else
  ISO="/nix/store/2fsacbs6jycpm0ajz2fr5ycgmagsjb5f-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso"
fi
DEST="/mnt/c/Users/msrov/Downloads/nixaios-minimal-26.11.20261020-x86_64-linux.iso"
ls -lh "$ISO"
cp -v "$ISO" "$DEST"
ls -lh "$DEST"
echo "OK -> $DEST"
