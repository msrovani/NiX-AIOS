#!/usr/bin/env bash
# V8.1 (plano vampiro) — smoke do `nosmcp` (MCP stdio read-only).
# Assume `nosd` a correr. stdout = ground truth; exit !=0 se falhar.
#
#   NOSD_TCP=127.0.0.1:7700 NOSD_IPC_TOKEN_FILE=./data/ipc.token \
#     bash scripts/smoke-nosmcp.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NOSMCP="${NOSMCP:-$REPO_ROOT/packages/nosd/target/debug/nosmcp}"
TOKEN_FILE="${NOSD_IPC_TOKEN_FILE:-/var/lib/nosd/ipc.token}"

ARGS=()
if [[ -n "${NOSD_TCP:-}" ]]; then
  ARGS=(--tcp "$NOSD_TCP")
else
  ARGS=(--socket "${NOSD_SOCKET:-/run/nosd/nosd.sock}")
fi
if [[ -f "$TOKEN_FILE" ]]; then
  ARGS+=(--token-file "$TOKEN_FILE")
fi

fail() { echo "FAIL: $*" >&2; exit 1; }
has() { grep -q -- "$1" <<<"$2" || fail "esperava '$1' em: ${2:0:240}"; }

echo "== initialize + tools/list + call + ping =="
out="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
    '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nsd.health","arguments":{}}}' \
    '{"jsonrpc":"2.0","id":4,"method":"ping"}' |
    "$NOSMCP" "${ARGS[@]}"
)"
has '"protocolVersion"' "$out"
has 'nsd.health' "$out"
has 'nsd.energy' "$out"
has 'engine' "$out"
has '"id":4' "$out"

echo "== read-only: nenhuma tool de escrita =="
for bad in write apply spawn set_transmute nix. shell; do
  if grep -q -- "$bad" <<<"$out"; then
    fail "tools/list expôs '$bad' (tem de ser read-only)"
  fi
done

echo "== parse error (-32700, id null) =="
out="$(printf 'nao-e-json\n{"jsonrpc":"2.0","id":9,"method":"ping"}\n' | "$NOSMCP" "${ARGS[@]}")"
has '"code":-32700' "$out"
has '"id":null' "$out"
has '"id":9' "$out"

echo "== tool desconhecida =="
out="$(printf '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nsd.write"}}\n' | "$NOSMCP" "${ARGS[@]}")"
has 'unknown tool' "$out"

echo "== trace (leitura, redigido) =="
out="$(printf '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"nsd.trace","arguments":{"last":2}}}\n' | "$NOSMCP" "${ARGS[@]}")"
has 'redaction' "$out"

echo "PASS: nosmcp (stdio, read-only)"
