#!/usr/bin/env bash
set -euo pipefail
source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null || true
nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
export PATH="/tmp/nos-mtools/bin:$PATH"

# 1) Stick Windows montado
for d in /mnt/[d-z] /mnt/[D-Z]; do
  [ -d "$d" ] || continue
  if [ -f "$d/firstboot.log" ] || [ -f "$d/hotswap.log" ] || [ -d "$d/nosd-persist" ]; then
    echo "=== STICK $d ==="
    ls -la "$d"
    for f in firstboot.log hotswap.log jarbas-cage.log journal-boot.log journal-follow.log; do
      if [ -f "$d/$f" ]; then
        echo "===== $f ====="
        tail -n 100 "$d/$f"
      fi
    done
    ls -la "$d/nosd-persist" 2>/dev/null || true
    for f in locale.conf aios-mode firstboot.done firstboot.step network.conf; do
      [ -f "$d/nosd-persist/$f" ] && echo "-- $f --" && cat "$d/nosd-persist/$f"
    done
    exit 0
  fi
done

echo "STICK_NOT_MOUNTED — a espreitar img Downloads (pode estar virgem)"
IMG=/mnt/c/Users/msrov/Downloads/nixaios-text-s525-transmute-persist.img
OFF=$((5609472 * 512))
mdir -i "${IMG}@@${OFF}" ::
for f in firstboot.log hotswap.log jarbas-cage.log; do
  rm -f "/tmp/$f"
  if mcopy -n -i "${IMG}@@${OFF}" "::/$f" "/tmp/$f" 2>/dev/null; then
    echo "===== img::$f ====="
    tail -n 80 "/tmp/$f"
  fi
done
mdir -i "${IMG}@@${OFF}" ::/nosd-persist 2>/dev/null || true
