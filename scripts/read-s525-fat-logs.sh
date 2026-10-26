#!/usr/bin/env bash
# Le logs NIXAIOSLOG: stick montado no Windows OU particao FAT do .img
set -euo pipefail
IMG="${1:-/mnt/c/Users/msrov/Downloads/nixaios-text-s525-transmute-persist.img}"
FILES="firstboot.log hotswap.log jarbas-cage.log journal-boot.log journal-follow.log transmute-launch.log wizard-watch.log README.txt"

echo "=== procura stick montado ==="
found=0
for d in /mnt/[d-z] /mnt/[D-Z]; do
  [ -d "$d" ] || continue
  if [ -f "$d/firstboot.log" ] || [ -f "$d/hotswap.log" ] || [ -d "$d/nosd-persist" ] || [ -f "$d/README.txt" ]; then
    echo "FOUND mount=$d"
    found=1
    ls -la "$d" | head -40
    for f in $FILES; do
      if [ -f "$d/$f" ]; then
        echo ""
        echo "========== $d/$f (tail 120) =========="
        tail -n 120 "$d/$f" || true
      fi
    done
    if [ -d "$d/nosd-persist" ]; then
      echo ""
      echo "========== nosd-persist =========="
      ls -la "$d/nosd-persist" || true
      for f in locale.conf aios-mode firstboot.done firstboot.step network.conf; do
        [ -f "$d/nosd-persist/$f" ] && echo "--- $f ---" && cat "$d/nosd-persist/$f"
      done
    fi
  fi
done

if [ "$found" = "1" ]; then
  exit 0
fi

echo "=== sem stick montado — le FAT do img ==="
echo "IMG=$IMG"
[ -f "$IMG" ] || { echo "IMG missing"; exit 1; }

# Particao 3 = FAT (apos ISO alinhada). Usa mtools se disponivel.
START=$(sfdisk -d "$IMG" 2>/dev/null | awk '/img3|Id=c|Id=0c|type=c|type=0c/ {print $4}' | tr -d ',' | head -1 || true)
# Fallback: calculo tipico s525 — ISO ~2.7G alinhado
if [ -z "${START:-}" ]; then
  # do pack script: START_SECT = ALIGN/512
  ISO_HINT=2872049664
  START=$((ISO_HINT / 512))
fi
echo "FAT start_sector=$START"

# extrai FAT para tmp e lista
FAT_TMP=/tmp/nixaios-s525-fat.img
OFFSET=$((START * 512))
# tamanho 4GiB
dd if="$IMG" of="$FAT_TMP" bs=512 skip="$START" count=$((4*1024*1024*1024/512)) status=progress 2>&1 | tail -5

export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:$PATH"
if ! command -v mdir >/dev/null; then
  nix build --extra-experimental-features 'nix-command flakes' nixpkgs#dosfstools --out-link /tmp/nos-dosfstools
  nix build --extra-experimental-features 'nix-command flakes' nixpkgs#mtools --out-link /tmp/nos-mtools
  export PATH="/tmp/nos-dosfstools/bin:/tmp/nos-mtools/bin:$PATH"
fi

echo "=== mdir :: ==="
mdir -i "$FAT_TMP" :: || true

for f in $FILES; do
  if mdir -i "$FAT_TMP" "::/$f" >/dev/null 2>&1; then
    echo ""
    echo "========== ::/$f =========="
    mcopy -n -i "$FAT_TMP" "::/$f" "/tmp/fat-$f" 2>/dev/null || true
    if [ -f "/tmp/fat-$f" ]; then
      wc -c "/tmp/fat-$f"
      tail -n 150 "/tmp/fat-$f"
    fi
  fi
done

echo ""
echo "=== nosd-persist ==="
mdir -i "$FAT_TMP" ::/nosd-persist 2>/dev/null || mdir -i "$FAT_TMP" ::/NOSD-PERSIST 2>/dev/null || true
for f in locale.conf aios-mode firstboot.done firstboot.step network.conf wifi-ssid; do
  mcopy -n -i "$FAT_TMP" "::/nosd-persist/$f" "/tmp/fat-persist-$f" 2>/dev/null \
    || mcopy -n -i "$FAT_TMP" "::/NOSD-PERSIST/$f" "/tmp/fat-persist-$f" 2>/dev/null \
    || true
  if [ -f "/tmp/fat-persist-$f" ]; then
    echo "--- $f ---"
    cat "/tmp/fat-persist-$f"
  fi
done
