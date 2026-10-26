#!/usr/bin/env bash
# ISO s526: mount robusto NIXAIOSLOG + firstboot grava GUEST_MOUNTED/firstboot.log
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_ISO="/mnt/c/Users/msrov/Downloads/nixaios-text-s526-transmute.iso"
DEST_IMG="/mnt/c/Users/msrov/Downloads/nixaios-text-s526-transmute-persist.img"
sed -i 's/\r$//' "$ROOT/scripts/build-iso-tmp.sh" "$ROOT/scripts/pack-usb-logfat.sh" || true
chmod +x "$ROOT/scripts/pack-usb-logfat.sh" "$ROOT/scripts/build-iso-tmp.sh"
# force volume id in synced tree (iso.nix)
sed -i 's/NIXAIOS525/NIXAIOS526/g' "$ROOT/images/iso.nix" || true
export LOG="/tmp/nixaios-iso-build-s526.log"
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee /tmp/nixaios-iso-s526.out
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
[[ -n "$ISO" && -f "$ISO" ]] || { echo "NO_ISO"; exit 1; }
cp -f "$ISO" "$DEST_ISO"
sha256sum "$DEST_ISO" | tee /tmp/nixaios-s526-iso.sha256
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#dosfstools --out-link /tmp/nos-dosfstools
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:/usr/sbin:/usr/bin:$PATH"
bash "$ROOT/scripts/pack-usb-logfat.sh" "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_IMG" | tee /tmp/nixaios-s526-img.sha256
python3 -c "from pathlib import Path;p=Path('$DEST_IMG');f=p.open('rb');f.seek(32768);print('pvd',f.read(2048)[40:72].decode('ascii','replace').strip())"
echo "COPIED_IMG=$DEST_IMG VOLUME=NIXAIOS526"
