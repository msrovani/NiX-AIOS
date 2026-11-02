#!/usr/bin/env bash
# Smoke host: sementes unicast do mesh (M5 frota).
#
# Duas instâncias reais do nosd com o broadcast apontado a uma porta FECHADA:
# o único caminho possível para o HB é a semente unicast. Falha (rc=1) se o
# log não mostrar targets=2/peers>=1 ou se o health não reportar o seed.
#
# Uso:
#   bash scripts/smoke-mesh-unicast.sh
#   NOSD_BIN=result/bin/nosd NOSCTL_BIN=... bash scripts/smoke-mesh-unicast.sh
#
# Requer o binário já construído (cargo build -p nosd --bins) e GNU grep.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${NOSD_BIN:-$ROOT/packages/nosd/target/debug/nosd}"
CTL="${NOSCTL_BIN:-$(dirname "$BIN")/nosctl}"
A="${TMPDIR:-/tmp}/nixaios-mesh-a"
B="${TMPDIR:-/tmp}/nixaios-mesh-b"
TCP_A="${SMOKE_TCP_A:-17800}"
TCP_B="${SMOKE_TCP_B:-17801}"
UDP_A="${SMOKE_UDP_A:-19110}"
UDP_B="${SMOKE_UDP_B:-19111}"
DEAD="${SMOKE_DEAD_UDP:-19199}"

fail() {
  echo "SMOKE_MESH_FAIL: $*" >&2
  exit 1
}

[[ -x "$BIN" ]] || fail "nosd não encontrado/executável: $BIN (cargo build -p nosd --bins)"
[[ -x "$CTL" ]] || fail "nosctl não encontrado/executável: $CTL"

rm -rf "$A" "$B"
mkdir -p "$A" "$B"
PA=""
PB=""
cleanup() {
  [[ -n "$PA" ]] && kill "$PA" 2>/dev/null
  [[ -n "$PB" ]] && kill "$PB" 2>/dev/null
  [[ -n "$PA" ]] && wait "$PA" 2>/dev/null
  [[ -n "$PB" ]] && wait "$PB" 2>/dev/null
}
trap cleanup EXIT

"$BIN" --foreground --tcp "127.0.0.1:$TCP_A" --data-dir "$A" --node-id 15 \
  --hermes-tick-secs 1 --mesh-bind "0.0.0.0:$UDP_A" \
  --mesh-broadcast "127.0.0.1:$DEAD" --mesh-peer "127.0.0.1:$UDP_B" \
  >"$A/nosd.log" 2>&1 &
PA=$!
"$BIN" --foreground --tcp "127.0.0.1:$TCP_B" --data-dir "$B" --node-id 16 \
  --hermes-tick-secs 1 --mesh-bind "0.0.0.0:$UDP_B" \
  --mesh-broadcast "127.0.0.1:$DEAD" --mesh-peer "127.0.0.1:$UDP_A" \
  >"$B/nosd.log" 2>&1 &
PB=$!

# HB a cada 1 s; TOFU settle não é necessário para contar o peer.
for _ in $(seq 1 20); do
  grep -aq "peers=1" "$A/nosd.log" 2>/dev/null && break
  sleep 1
done

grep -aq "targets=2 peers=1" "$A/nosd.log" \
  || fail "A não viu targets=2/peers=1 (log: $A/nosd.log)"
grep -aq "mesh HB peer=16" "$A/nosd.log" \
  || fail "A não recebeu HB do peer 16 pela semente (log: $A/nosd.log)"

"$CTL" --tcp "127.0.0.1:$TCP_A" --token-file "$A/ipc.token" health \
  >"$A/health.json" 2>&1 || fail "health de A falhou"
grep -aq '"mesh_peers": 1' "$A/health.json" \
  || fail "health sem mesh_peers=1 ($A/health.json)"
grep -aq "127.0.0.1:$UDP_B" "$A/health.json" \
  || fail "health não mostra o seed configurado ($A/health.json)"

echo "SMOKE_MESH_OK targets=2 peers=1 seed=127.0.0.1:$UDP_B"
