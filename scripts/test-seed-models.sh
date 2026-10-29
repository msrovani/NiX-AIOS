#!/usr/bin/env bash
# Teste do seed offline (modules/transmute/seed-models.sh) sem FAT real.
# Corre com: bash scripts/test-seed-models.sh
# No check Nix: SEED=<store path> bash scripts/test-seed-models.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SEED="${SEED:-$ROOT/modules/transmute/seed-models.sh}"
[ -f "$SEED" ] || { echo "FAIL: seed script ausente: $SEED"; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
MNT="$TMP/fat"
DEST="$TMP/models"
mkdir -p "$MNT/models"

run() {
  MNT="$MNT" DEST="$DEST" bash "$SEED" > "$1" 2>&1
}

# 1. copia models/*.gguf
printf 'GGUF' > "$MNT/models/active-test.gguf"
run "$TMP/1.log"
[ -f "$DEST/active-test.gguf" ] || { echo "FAIL: nao copiou models/*.gguf"; exit 1; }
grep -q 'OK ' "$TMP/1.log" || { echo "FAIL: sem linha OK"; exit 1; }

# 2. idempotente (mesmo tamanho) nao re-copia
run "$TMP/2.log"
grep -q 'ja presente active-test.gguf' "$TMP/2.log" \
  || { echo "FAIL: nao detectou ficheiro ja presente"; exit 1; }

# 3. ignora o que nao e .gguf
printf 'x' > "$MNT/models/nao-modelo.bin"
run "$TMP/3.log"
[ ! -f "$DEST/nao-modelo.bin" ] || { echo "FAIL: copiou .bin"; exit 1; }

# 4. aceita .gguf na raiz da FAT
printf 'G2' > "$MNT/root-extra.gguf"
run "$TMP/4.log"
[ -f "$DEST/root-extra.gguf" ] || { echo "FAIL: nao copiou .gguf da raiz"; exit 1; }

# 5. sem FAT -> exit 0 + aviso (nao falha o boot)
MNT="$TMP/ausente" DEST="$DEST" bash "$SEED" > "$TMP/5.log" 2>&1
grep -q 'FAT ausente' "$TMP/5.log" || { echo "FAIL: sem aviso de FAT ausente"; exit 1; }

# 6. actualiza quando o tamanho muda (modelo novo no stick)
printf 'GGUF-v2' > "$MNT/models/active-test.gguf"
run "$TMP/6.log"
cmp -s "$MNT/models/active-test.gguf" "$DEST/active-test.gguf" \
  || { echo "FAIL: nao actualizou modelo com tamanho diferente"; exit 1; }

echo "test-seed-models: PASS"
