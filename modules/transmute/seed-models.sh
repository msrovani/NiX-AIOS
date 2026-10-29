#!/usr/bin/env bash
# Copia GGUF de $MNT/models (ou da raiz de $MNT) para $DEST.
# Invocado por nixaios-model-seed.service (modules/log-fat32.nix).
# Sem FAT nao e fatal: o boot continua e o modelo pode vir da rede.
set -eu
MNT="${MNT:-/mnt/nixaios-log}"
DEST="${DEST:-/var/lib/nosd/models}"
say() {
  echo "nixaios-model-seed: $*"
  echo "nixaios-model-seed: $*" > /dev/tty1 2>/dev/null || true
}
if [ ! -d "$MNT" ]; then
  say "FAT ausente — sem seed offline (modelo teria de vir da rede)"
  exit 0
fi
mkdir -p "$DEST"
if [ ! -w "$DEST" ]; then
  say "ERRO: $DEST nao gravavel"
  exit 0
fi
found=0
for src in "$MNT"/models/*.gguf "$MNT"/*.gguf; do
  [ -f "$src" ] || continue
  name=$(basename "$src")
  case "$name" in
    *.gguf) ;;
    *) continue ;;
  esac
  found=1
  dst="$DEST/$name"
  ssz=$(stat -c %s "$src" 2>/dev/null || echo 0)
  dsz=$(stat -c %s "$dst" 2>/dev/null || echo 0)
  if [ "$ssz" -gt 0 ] && [ "$ssz" = "$dsz" ]; then
    say "ja presente $name ($ssz bytes)"
    continue
  fi
  avail=$(df -Pk "$DEST" | awk 'NR==2 {print $4 * 1024}')
  if [ "${avail:-0}" -lt "$ssz" ]; then
    say "ERRO: sem espaco para $name ($ssz bytes; livre=${avail:-0})"
    continue
  fi
  say "a copiar $name ($ssz bytes) -> $dst"
  if cp -f "$src" "$dst.part" && mv -f "$dst.part" "$dst"; then
    sync
    say "OK $dst"
  else
    rm -f "$dst.part" 2>/dev/null || true
    say "ERRO a copiar $name"
  fi
done
if [ "$found" = 0 ]; then
  say "sem .gguf em $MNT/models (opcional: modelo pode vir da rede)"
fi
{
  echo "=== model-seed $(date -u +%FT%TZ) ==="
  ls -la "$DEST" 2>&1 || true
} >> "$MNT/model-seed.log" 2>&1 || true
sync
