#!/usr/bin/env bash
set -euo pipefail
ISO="/nix/store/2fsacbs6jycpm0ajz2fr5ycgmagsjb5f-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso"
DEST="/mnt/c/Users/msrov/Downloads/nixaios-minimal-onda3-26.11.iso"
ls -lh "$ISO"
rm -f "$DEST" 2>/dev/null || true
cp -f "$ISO" "$DEST"
ls -lh "$DEST"
echo "OK $DEST"
