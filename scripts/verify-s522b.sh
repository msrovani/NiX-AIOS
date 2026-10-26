#!/usr/bin/env bash
set -euo pipefail
LOG=/tmp/nixaios-iso-build-s522.log
SYS=$(grep -oE '/nix/store/[a-z0-9]+-nixos-system-nixaios[^ /"]*' "$LOG" 2>/dev/null | head -1 || true)
echo "SYS=$SYS"
if [ -z "$SYS" ]; then
  SYS=$(ls -d /nix/store/*-nixos-system-nixaios-* 2>/dev/null | tail -1 || true)
  echo "SYS_fallback=$SYS"
fi
if [ -n "$SYS" ] && [ -d "$SYS" ]; then
  ls -la "$SYS/sw/bin/nos-firstboot" "$SYS/sw/bin/nos-transmute" 2>&1 || true
  ls "$SYS/etc/profile.d/" 2>&1 | head -20 || true
  if [ -f "$SYS/sw/bin/nos-firstboot" ]; then
    echo "--- nos-firstboot head ---"
    head -20 "$SYS/sw/bin/nos-firstboot"
  fi
  # getty autologin
  grep -R "autologin" "$SYS/etc" 2>/dev/null | head -10 || true
  test -f "$SYS/etc/profile.d/nos-firstboot.sh" && echo "profile.d OK" || echo "profile.d MISS"
fi
FB=$(find /nix/store -maxdepth 2 -type d -name 'nos-firstboot-*' 2>/dev/null | head -5 || true)
echo "FB_dirs=$FB"
