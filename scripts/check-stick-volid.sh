#!/usr/bin/env bash
# Compare ISO9660 volume ID on USB vs Downloads s515 ISO.
set -euo pipefail
ISO="${ISO:-/mnt/c/Users/msrov/Downloads/nixaios-minimal-s515-webcam-cinema.iso}"

volid_of() {
  local dev="$1"
  dd if="$dev" bs=1 skip=$((16 * 2048 + 40)) count=32 2>/dev/null | tr -d ' \0'
}

echo "==> ISO file"
ls -lh "$ISO"
echo "ISO_VOL=[$(volid_of "$ISO")]"
echo "ISO_SHA=$(sha256sum "$ISO" | awk '{print $1}')"

echo
echo "==> block devices"
lsblk -o NAME,SIZE,TYPE,LABEL,FSTYPE,TRAN,MODEL 2>/dev/null || lsblk

echo
echo "==> candidates (USB / removable)"
# Prefer sdX that is not the WSL system disk
for d in /dev/sd[a-z] /dev/sd[a-z][0-9]; do
  [ -b "$d" ] || continue
  # skip partitions for volid — need whole disk for ISO9660 PVD
  case "$d" in
    *[0-9]) continue ;;
  esac
  model=$(lsblk -ndo MODEL "$d" 2>/dev/null || true)
  tran=$(lsblk -ndo TRAN "$d" 2>/dev/null || true)
  size=$(lsblk -ndo SIZE "$d" 2>/dev/null || true)
  vol=$(volid_of "$d" || true)
  echo "DISK=$d size=$size tran=$tran model=[$model] VOL=[$vol]"
  blkid "$d" 2>/dev/null || true
done

echo
echo "Expect VOL=NIXAIOS515 for s515 webcam cinema."
echo "On live stick: blkid | grep -i nixaios ; journalctl -u jarbas-cage -b --no-pager | tail -80"
