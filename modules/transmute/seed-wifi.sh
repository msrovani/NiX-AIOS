#!/usr/bin/env bash
# Liga o Wi-Fi a partir de $MNT/wifi.conf:
#   SSID=MinhaRede
#   PSK=segredo
#   HIDDEN=1   (opcional)
# Chamado por nixaios-wifi-seed.service. Falha nao e fatal (o boot continua).
set -eu
MNT="${MNT:-/mnt/nixaios-log}"
DIAG="$MNT/wifi-diag.log"
say() { echo "nixaios-wifi-seed: $*"; }
diag() {
  {
    echo "=== wifi-seed $(date -u +%FT%TZ 2>/dev/null || echo now) ==="
    "$@"
  } >> "$DIAG" 2>&1 || true
}
if [ ! -f "$MNT/wifi.conf" ]; then
  say "sem $MNT/wifi.conf (Wi-Fi fica para o utilizador)"
  exit 0
fi
SSID=""
PSK=""
HIDDEN=0
while IFS= read -r line || [ -n "$line" ]; do
  line="${line%$'\r'}"
  case "$line" in
    ''|'#'*) continue ;;
  esac
  key="${line%%=*}"
  val="${line#*=}"
  case "$key" in
    SSID) SSID="$val" ;;
    PSK) PSK="$val" ;;
    HIDDEN) HIDDEN="$val" ;;
  esac
done < "$MNT/wifi.conf"
if [ -z "$SSID" ]; then
  say "wifi.conf sem SSID — ignorado"
  exit 0
fi
rfkill unblock all 2>/dev/null || true
nmcli radio wifi on 2>/dev/null || true
say "a ligar SSID=$SSID"
set -- device wifi connect "$SSID"
if [ -n "$PSK" ]; then
  set -- "$@" password "$PSK"
fi
if [ "$HIDDEN" = "1" ]; then
  set -- "$@" hidden yes
fi
if nmcli "$@"; then
  say "Wi-Fi OK ($SSID)"
  echo "wifi-seed OK SSID=$SSID $(date -u +%FT%TZ 2>/dev/null || echo now)" >> "$DIAG" 2>/dev/null || true
else
  say "Wi-Fi FALHOU ($SSID) — ver wifi-diag.log"
  diag nmcli -f DEVICE,TYPE,STATE device
  diag nmcli -f SSID,SIGNAL,SECURITY device wifi list
  exit 0
fi
