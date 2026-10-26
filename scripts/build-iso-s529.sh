#!/usr/bin/env bash
# Alias legado → 0.5.1 (NIXAIOS051). Preferir scripts/build-iso-050.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
echo "build-iso-s529.sh: redirect → build-iso-050.sh (NixAIOS 0.5.1)"
exec bash "$ROOT/scripts/build-iso-050.sh"
sed -i 's/\r$//' "$ROOT"/scripts/*.sh "$ROOT"/images/iso.nix "$ROOT"/modules/*.nix \
  "$ROOT"/packages/nosd/crates/nos-jarbas/ui/hud.slint \
  "$ROOT"/packages/nosd/crates/nos-jarbas/src/slint_app.rs 2>/dev/null || true
chmod +x "$ROOT/scripts/pack-usb-logfat.sh" "$ROOT/scripts/build-iso-tmp.sh"
export LOG="/tmp/nixaios-iso-build-s529.log"
bash "$ROOT/scripts/build-iso-tmp.sh" 2>&1 | tee /tmp/nixaios-iso-s529.out
OUT=$(readlink -f /tmp/nixaios-flake/result || true)
ISO=$(find "$OUT" -name '*.iso' -type f 2>/dev/null | head -1 || true)
[[ -n "$ISO" && -f "$ISO" ]] || { echo "NO_ISO out=$OUT"; exit 1; }
cp -f "$ISO" "$DEST_ISO"
bash "$ROOT/scripts/guard-iso-no-s1-blob.sh" "$DEST_ISO" || {
  echo "S1.2 REJECT guard failed — abort copy"
  exit 1
}
sha256sum "$DEST_ISO" | tee /tmp/nixaios-s529-iso.sha256
# DD sem FAT32 (s529): Rufus grava a ISO pura; persistência = overlay Linux /var/lib/nosd.
# (pack-usb-logfat.sh aposentado nesta linha — permissões VFAT no metal).
cp -f "$DEST_ISO" "$DEST_IMG"
sha256sum "$DEST_IMG" | tee /tmp/nixaios-s529-img.sha256
python3 -c "from pathlib import Path;p=Path(r'$DEST_IMG');f=p.open('rb');f.seek(32768);print('pvd',f.read(2048)[40:72].decode('ascii','replace').strip())"
echo "COPIED_ISO=$DEST_ISO"
echo "COPIED_IMG=$DEST_IMG VOLUME=NIXAIOS529 UI=slint remig=s529 NO_FAT32=1"
