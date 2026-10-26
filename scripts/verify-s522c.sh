#!/usr/bin/env bash
set -euo pipefail
# Resolve actual system from iso closure
ISO_OUT=$(readlink -f /tmp/nixaios-flake/result)
echo "ISO_OUT=$ISO_OUT"
# nix-store -q --references or --requisites
if command -v nix-store >/dev/null; then
  source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
  REFS=$(nix-store -qR "$ISO_OUT" 2>/dev/null | grep -E 'nos-firstboot|nixos-system-nixaios' | head -20 || true)
  echo "$REFS"
  FB=$(echo "$REFS" | grep nos-firstboot | head -1 || true)
  SYS=$(echo "$REFS" | grep 'nixos-system-nixaios' | grep -v '\.drv$' | head -1 || true)
  echo "FB=$FB"
  echo "SYS=$SYS"
  if [ -n "$FB" ]; then
    ls -la "$FB/bin" || ls -la "$FB"
    head -15 "$FB/bin/nos-firstboot" 2>/dev/null || head -15 "$FB"
  fi
  if [ -n "$SYS" ]; then
    test -f "$SYS/etc/profile.d/nos-firstboot.sh" && echo profile_d=OK || echo profile_d=MISS
    ls "$SYS/sw/bin/nos-firstboot" 2>&1 || true
    # console getty
    grep -l Autologin "$SYS"/etc/systemd/system/getty@.service.d/* 2>/dev/null || true
    find "$SYS/etc" -name '*autologin*' 2>/dev/null | head
    grep -R "nixos" "$SYS/etc/systemd/system/getty"* 2>/dev/null | head -10 || true
  fi
fi
