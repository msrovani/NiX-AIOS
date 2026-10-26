#!/usr/bin/env bash
# Captura serial QEMU — lab Ring3_Linux/qemu (≠ metal). Prefer kernel+initrd (pula menu).
set -euo pipefail
ISO="${ISO:-/mnt/c/Users/msrov/Downloads/nixaios-minimal-s529-remig.iso}"
LOG="${LOG:-/tmp/nixaios-qemu.log}"
SECS="${SECS:-360}"
MEM="${MEM:-3G}"
WORK="${WORK:-/tmp/nixaios-iso-boot}"

if [[ ! -f "$ISO" ]]; then
  echo "ISO missing: $ISO" >&2
  exit 1
fi

# shellcheck disable=SC1091
[[ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]] && \
  . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh

if [[ -e /dev/kvm ]] && [[ ! -r /dev/kvm || ! -w /dev/kvm ]]; then
  sudo -n chmod 666 /dev/kvm 2>/dev/null || true
fi

ACCEL=(-cpu max)
if [[ -e /dev/kvm && -r /dev/kvm && -w /dev/kvm ]]; then
  ACCEL=(-enable-kvm -cpu host)
  echo "==> KVM ON"
else
  echo "==> TCG (slow)"
fi

mkdir -p "$WORK"
if [[ ! -f "$WORK/bzImage" || ! -f "$WORK/initrd" || ! -f "$WORK/cmdline" ]]; then
  echo "==> extracting boot bits..."
  rm -rf "$WORK/extract"
  mkdir -p "$WORK/extract"
  nix-shell -p p7zip --run "7z x -y -o$WORK/extract '$ISO' 'boot' 'isolinux' 'EFI' 'loader'" >/tmp/nixaios-7z.out 2>&1 || true
  IMG=$(find "$WORK/extract" -type f -name 'bzImage' | head -1)
  IRD=$(find "$WORK/extract" -type f \( -name 'initrd' -o -name 'initrd.gz' \) | head -1)
  CFG=$(find "$WORK/extract" -type f \( -name 'isolinux.cfg' -o -name 'grub.cfg' \) | head -1 || true)
  if [[ -z "${IMG:-}" || -z "${IRD:-}" ]]; then
    echo "extract failed — falling back to -cdrom only" >&2
    cat /tmp/nixaios-7z.out 2>/dev/null | tail -20 || true
    MODE=cdrom
  else
    cp -f "$IMG" "$WORK/bzImage"
    cp -f "$IRD" "$WORK/initrd"
    CMDLINE=""
    if [[ -n "${CFG:-}" ]] && grep -q 'APPEND' "$CFG"; then
      CMDLINE=$(grep -E 'APPEND' "$CFG" | head -1 | sed -E 's/.*APPEND[[:space:]]+//')
    fi
    [[ -z "$CMDLINE" ]] && CMDLINE="root=LABEL=NIXAIOS051 boot.shell_on_fail"
    case "$CMDLINE" in
      *console=ttyS0*) ;;
      *) CMDLINE="$CMDLINE console=ttyS0,115200n8" ;;
    esac
    printf '%s\n' "$CMDLINE" > "$WORK/cmdline"
    MODE=kernel
  fi
else
  MODE=kernel
fi

rm -f "$LOG"
echo "==> MODE=$MODE ISO=$ISO LOG=$LOG SECS=$SECS"
set +e
if [[ "$MODE" == "kernel" ]]; then
  CMDLINE=$(cat "$WORK/cmdline")
  echo "==> CMDLINE=$CMDLINE"
  timeout "$SECS" qemu-system-x86_64 \
    "${ACCEL[@]}" \
    -m "$MEM" \
    -smp 2 \
    -kernel "$WORK/bzImage" \
    -initrd "$WORK/initrd" \
    -append "$CMDLINE" \
    -cdrom "$ISO" \
    -boot order=d \
    -netdev user,id=net0 \
    -device virtio-net-pci,netdev=net0 \
    -nographic \
    -serial mon:stdio \
    >"$LOG" 2>&1
else
  timeout "$SECS" qemu-system-x86_64 \
    "${ACCEL[@]}" \
    -m "$MEM" \
    -smp 2 \
    -cdrom "$ISO" \
    -boot d \
    -netdev user,id=net0 \
    -device virtio-net-pci,netdev=net0 \
    -nographic \
    -serial mon:stdio \
    >"$LOG" 2>&1
fi
rc=$?
set -e
echo "QEMU_EXIT=$rc"
wc -c "$LOG" || true
echo "--- markers ---"
grep -Eic 'Linux version|systemd|NixOS|nosd|JARBAS|Welcome|login:|Reached target' "$LOG" || true
echo "--- tail ---"
tail -n 80 "$LOG" || true
