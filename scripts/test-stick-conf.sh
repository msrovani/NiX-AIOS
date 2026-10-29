#!/usr/bin/env bash
# Teste dos helpers de persistencia no pendrive (stick-conf.sh).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HELPERS="${HELPERS:-$ROOT/modules/transmute/stick-conf.sh}"
[ -f "$HELPERS" ] || { echo "FAIL: helpers ausentes: $HELPERS"; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export SUDO=""
STICK_DIR="$TMP"
STICK_CFG="$TMP/firstboot.conf"
STICK_OK=1
# shellcheck disable=SC1090
. "$HELPERS"

fail() { echo "FAIL: $*"; exit 1; }

# 1. leitura (inclui '=' no valor)
printf 'DONE=1\nLANG=pt_PT.UTF-8\nKEYMAP=us\nTZ=Europe/Lisbon\nPSK=a=b\n' > "$STICK_CFG"
[ "$(stick_get LANG)" = "pt_PT.UTF-8" ] || fail "LANG"
[ "$(stick_get KEYMAP)" = "us" ] || fail "KEYMAP"
[ "$(stick_get PSK)" = "a=b" ] || fail "PSK com ="
if stick_get NOPE >/dev/null 2>&1; then fail "chave ausente devia falhar"; fi

# 2. grava DONE=0 e valores
LANG_SEL=es_ES.UTF-8
KB=es
TZ=UTC
save_stick_cfg 0
grep -qx 'DONE=0' "$STICK_CFG" || fail "DONE=0"
grep -qx 'LANG=es_ES.UTF-8' "$STICK_CFG" || fail "LANG gravado"
grep -qx 'KEYMAP=es' "$STICK_CFG" || fail "KEYMAP gravado"
grep -qx 'TZ=UTC' "$STICK_CFG" || fail "TZ gravado"

# 3. DONE=1 no fim
save_stick_cfg 1
grep -qx 'DONE=1' "$STICK_CFG" || fail "DONE=1"

# 4. wifi.conf multi-linha
save_stick wifi.conf "SSID=Lab
PSK=s3cr3t"
grep -qx 'SSID=Lab' "$TMP/wifi.conf" || fail "wifi.conf SSID"
grep -qx 'PSK=s3cr3t' "$TMP/wifi.conf" || fail "wifi.conf PSK"

# 5. sem FAT -> no-op
STICK_OK=0
STICK_CFG="$TMP/nao-existe.conf"
save_stick_cfg 1
[ ! -e "$TMP/nao-existe.conf" ] || fail "gravou sem FAT"
if stick_get LANG >/dev/null 2>&1; then fail "leu sem FAT"; fi

echo "test-stick-conf: PASS"
