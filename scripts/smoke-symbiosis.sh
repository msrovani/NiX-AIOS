#!/usr/bin/env bash
# Simbiose (2026-11-03) — smoke de bancada: domínio Nix + fábrica Rust/Python.
#
# Assume `nosd` a correr:
#   NOSD_TCP=127.0.0.1:7700 NOSD_IPC_TOKEN_FILE=./data/ipc.token \
#     bash scripts/smoke-symbiosis.sh
#
# stdout = ground truth; exit ≠0 se uma asserção falhar.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NOSCTL="${NOSCTL:-}"
if [[ -z "$NOSCTL" ]]; then
  if command -v nosctl >/dev/null 2>&1; then
    NOSCTL="$(command -v nosctl)"
  else
    NOSCTL="$REPO_ROOT/packages/nosd/target/debug/nosctl"
  fi
fi

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

run() { "$NOSCTL" "${ARGS[@]}" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
# Espaçamento do serde_json pretty não é contrato (lição 25).
has() { grep -Eq "$1" <<<"$2" || fail "esperava /$1/ em: ${2:0:280}"; }

echo "== S1 nix-status =="
out="$(run nix-status)"
has '"facts"' "$out"
has '"line"' "$out"
has '"lessons": [0-9]+' "$out"

echo "== S2 nix-ask (lição real, não folclore) =="
out="$(run nix-ask "nix build falha porque o flake não vê o ficheiro novo")"
has '"match": [1-9]' "$out"
has 'flake-git-tree' "$out"

echo "== S3 nix-ask sem relação = zero lições =="
out="$(run nix-ask "qual é a capital de França")"
has '"match": 0' "$out"

echo "== S4 symbiosis (o mesmo que o HUD vê) =="
out="$(run symbiosis)"
has '"nix"' "$out"
has '"factory"' "$out"
has '"honesty"' "$out"

echo "== S5 factory-scaffold rust =="
out="$(run factory-scaffold "app rust de saudação" --lang rust)"
has '"lang": "rust"' "$out"
has '"flake.nix"' "$out"
has '"content_id": "[0-9a-f]{64}"' "$out"

echo "== S6 factory-scaffold python =="
out="$(run factory-scaffold "app python de saudação" --lang python)"
has '"lang": "python"' "$out"
has 'app.py' "$out"

echo "== S7 factory-build rust (constrói e corre) =="
out="$(run factory-build "app rust de saudação" --lang rust)"
has '"status": "ok"' "$out"
has '"lang": "rust"' "$out"

echo "== S8 factory-build python (constrói e corre) =="
out="$(run factory-build "app python de saudação" --lang python)"
has '"status": "ok"' "$out"
has '"lang": "python"' "$out"
has 'app\.py' "$out"

echo "PASS: simbiose (nix.status/nix.ask/orb.symbiosis/factory.scaffold/factory.build)"
