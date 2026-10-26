#!/usr/bin/env bash
# J-01/M3: aceite mínimo Jarvis-like (host/VM). Não substitui QEMU/metal.
# Uso:
#   bash scripts/accept-m3.sh
#   NOSD_SOCKET=/run/nosd/nosd.sock bash scripts/accept-m3.sh
#   NOSD_TCP=127.0.0.1:7700 NOSD_IPC_TOKEN_FILE=./data/ipc.token bash scripts/accept-m3.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NOSCTL="${NOSCTL:-}"
if [[ -z "$NOSCTL" ]]; then
  if command -v nosctl >/dev/null 2>&1; then
    NOSCTL="$(command -v nosctl)"
  elif [[ -x "$ROOT/packages/nosd/target/debug/nosctl" ]]; then
    NOSCTL="$ROOT/packages/nosd/target/debug/nosctl"
  elif [[ -x "$ROOT/packages/nosd/target/release/nosctl" ]]; then
    NOSCTL="$ROOT/packages/nosd/target/release/nosctl"
  else
    echo "FAIL: nosctl not found (build packages/nosd first)" >&2
    exit 2
  fi
fi

SOCK="${NOSD_SOCKET:-}"
if [[ -z "$SOCK" && -S /run/nosd/nosd.sock ]]; then
  SOCK=/run/nosd/nosd.sock
fi
TOKEN_FILE="${NOSD_IPC_TOKEN_FILE:-${NOSD_DATA_DIR:-/var/lib/nosd}/ipc.token}"
TCP="${NOSD_TCP:-127.0.0.1:7700}"

rpc() {
  if [[ -n "$SOCK" && -S "$SOCK" ]]; then
    "$NOSCTL" --socket "$SOCK" --token-file "$TOKEN_FILE" "$@" 2>/dev/null
  else
    "$NOSCTL" --tcp "$TCP" --token-file "$TOKEN_FILE" "$@" 2>/dev/null
  fi
}

fail=0
step() { printf '%-24s ' "$1"; }
ok() { echo "PASS ${2:-}"; }
bad() { echo "FAIL ${2:-}"; fail=$((fail+1)); }

step "health"
H="$(rpc health || true)"
if echo "$H" | grep -qiE '"status"[[:space:]]*:[[:space:]]*"ok"'; then ok; else bad "$H"; fi

step "route"
R="$(rpc route "nvidia pci driver" || true)"
if echo "$R" | grep -qiE '"(slot|decision|s1)"'; then ok; else bad "$R"; fi

step "chat"
C="$(rpc chat "oi nixaios" || true)"
if echo "$C" | grep -qiE '"ok"[[:space:]]*:[[:space:]]*true' &&
   echo "$C" | grep -qiE '"(text|reply)"'; then ok; else bad "$C"; fi

step "remember/recall"
M="$(rpc recall "nixaios" || true)"
if echo "$M" | grep -qiE '"ok"[[:space:]]*:[[:space:]]*true|"records"|"hits"'; then ok; else bad "$M"; fi

step "voice-status"
V="$(rpc voice-status || true)"
if echo "$V" | grep -qiE 'EngineAbsent|EngineReady|"posture"|Ready'; then
  ok "(Ready|Absent honesto)"
else
  bad "$V"
fi

step "ui.card"
U="$(rpc ui-card "M3 acceptance" --kv "stage=M3,adr=0058" || true)"
if echo "$U" | grep -qiE '"window"|"card"'; then ok; else bad "$U"; fi

echo "---"
if [[ "$fail" -gt 0 ]]; then
  echo "accept-m3: FAIL fails=$fail target=${SOCK:-$TCP}"
  exit 1
fi
echo "accept-m3: PASS target=${SOCK:-$TCP}"
