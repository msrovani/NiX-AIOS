#!/usr/bin/env bash
# Teste do Wi-Fi por pendrive (modules/transmute/seed-wifi.sh) com nmcli fake.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SEED="${SEED:-$ROOT/modules/transmute/seed-wifi.sh}"
[ -f "$SEED" ] || { echo "FAIL: seed script ausente: $SEED"; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
MNT="$TMP/fat"
BIN="$TMP/bin"
NMCLI_LOG="$TMP/nmcli.log"
mkdir -p "$MNT" "$BIN"
export NMCLI_LOG

BASH_BIN="$(command -v bash)"
{
  printf '#!%s\n' "$BASH_BIN"
  cat <<'EOF'
echo "$*" >> "$NMCLI_LOG"
case "$*" in
  *"device wifi connect"*) exit "${FAKE_RC:-0}" ;;
  *) echo "fake nmcli: $*"; exit 0 ;;
esac
EOF
} > "$BIN/nmcli"
{
  printf '#!%s\n' "$BASH_BIN"
  echo 'exit 0'
} > "$BIN/rfkill"
chmod +x "$BIN/nmcli" "$BIN/rfkill"

run() {
  MNT="$MNT" PATH="$BIN:$PATH" bash "$SEED" > "$1" 2>&1
}

# 1. sem wifi.conf -> exit 0 e nao chama nmcli connect
: > "$NMCLI_LOG"
run "$TMP/1.log"
grep -q 'sem .*wifi.conf' "$TMP/1.log" || { echo "FAIL: sem aviso de wifi.conf"; exit 1; }
! grep -q 'wifi connect' "$NMCLI_LOG" || { echo "FAIL: chamou nmcli sem conf"; exit 1; }

# 2. SSID + PSK
printf 'SSID=Lab\nPSK=s3cr3t\n' > "$MNT/wifi.conf"
: > "$NMCLI_LOG"
run "$TMP/2.log"
grep -q 'Wi-Fi OK' "$TMP/2.log" || { echo "FAIL: nao ligou"; cat "$TMP/2.log"; exit 1; }
grep -q 'device wifi connect Lab password s3cr3t' "$NMCLI_LOG" \
  || { echo "FAIL: argumentos nmcli errados: $(cat "$NMCLI_LOG")"; exit 1; }

# 3. CRLF (ficheiro criado no Windows) + PSK com '=' + HIDDEN
printf 'SSID=Casa\r\nPSK=pa=ss\r\nHIDDEN=1\r\n' > "$MNT/wifi.conf"
: > "$NMCLI_LOG"
run "$TMP/3.log"
grep -q 'device wifi connect Casa password pa=ss hidden yes' "$NMCLI_LOG" \
  || { echo "FAIL: CRLF/PSK/hidden: $(cat "$NMCLI_LOG")"; exit 1; }

# 4. sem PSK -> rede aberta (sem 'password')
printf 'SSID=Open\n' > "$MNT/wifi.conf"
: > "$NMCLI_LOG"
run "$TMP/4.log"
grep -q 'device wifi connect Open$' "$NMCLI_LOG" \
  || { echo "FAIL: rede aberta: $(cat "$NMCLI_LOG")"; exit 1; }

# 5. falha de ligacao -> exit 0 + diagnostico
printf 'SSID=Errada\nPSK=x\n' > "$MNT/wifi.conf"
: > "$NMCLI_LOG"
FAKE_RC=1 MNT="$MNT" PATH="$BIN:$PATH" bash "$SEED" > "$TMP/5.log" 2>&1
grep -q 'Wi-Fi FALHOU' "$TMP/5.log" || { echo "FAIL: sem aviso de falha"; exit 1; }
grep -q 'wifi-seed' "$MNT/wifi-diag.log" || { echo "FAIL: sem diag"; exit 1; }

# 6. conf sem SSID -> ignorado
printf 'PSK=x\n' > "$MNT/wifi.conf"
: > "$NMCLI_LOG"
run "$TMP/6.log"
grep -q 'sem SSID' "$TMP/6.log" || { echo "FAIL: sem aviso de SSID"; exit 1; }
! grep -q 'wifi connect' "$NMCLI_LOG" || { echo "FAIL: ligou sem SSID"; exit 1; }

echo "test-seed-wifi: PASS"
