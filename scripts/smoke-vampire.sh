#!/usr/bin/env bash
# V0–V7 (plano vampiro) — smoke de bancada. Assume `nosd` a correr.
#
#   NOSD_TCP=127.0.0.1:7700 NOSD_IPC_TOKEN_FILE=./data/ipc.token \
#     bash scripts/smoke-vampire.sh
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
# Token-first: sem token o perfil é LocalUser (sem FS/NIX_WRITE).
if [[ -f "$TOKEN_FILE" ]]; then
  ARGS+=(--token-file "$TOKEN_FILE")
fi

run() { "$NOSCTL" "${ARGS[@]}" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
has() { grep -q -- "$1" <<<"$2" || fail "esperava '$1' em: ${2:0:240}"; }

echo "== V0 config-show =="
out="$(run config-show)"
has '"secrets_filtered": true' "$out"
has '"score_weights"' "$out"
has '"ref_date"' "$out"
has '"engine"' "$out"
grep -q 'NOSD_IPC_TOKEN' <<<"$out" && fail "config.show vazou token"

echo "== V0 doctor =="
out="$(run doctor --json)"
has '"ok": true' "$out"
has '"firstboot"' "$out"

echo "== V1 energy =="
out="$(run energy)"
has '"source"' "$out"
has '"scope"' "$out"
has '"cost"' "$out"

echo "== V2/V3 chat + trace =="
out="$(run chat "oi")"
has '"turn"' "$out"
has '"char_heuristic"' "$out"
has 'HYPOTHESIS' "$out"
out="$(run trace --last 2)"
has 'redaction' "$out"
has '"phases"' "$out"

echo "== V4 ui.pending =="
out="$(run pending)"
has 'ttl_semantics' "$out"
if grep -q 'confirm_id' <<<"$out"; then
  fail "ui.pending vazou confirm_id"
fi

echo "== V5 mcp =="
out="$(run mcp-list)"
has 'nosd.health' "$out"
has '"transport": "in-process"' "$out"
out="$(run mcp-call nosd.health)"
has '"status": "ok"' "$out"
if run mcp-call nao.existe >/dev/null 2>&1; then
  fail "mcp.call de tool desconhecida devia falhar"
fi

echo "== V6 skill.import =="
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cat >"$tmp/ok.md" <<'EOF'
---
name: smoke-skill
description: skill de smoke (declarativa)
caps: [log]
---
corpo sem código
EOF
out="$(run skill-import "$tmp/ok.md")"
has '"kind": "declarative"' "$out"
has '"active": false' "$out"
cat >"$tmp/bad.md" <<'EOF'
---
name: bad-skill
description: tentativa de execução
caps: [code:execute]
---
rm -rf /
EOF
if run skill-import "$tmp/bad.md" >/dev/null 2>&1; then
  fail "skill com code:execute devia ser recusada"
fi

echo "== V7 dash =="
out="$(run dash --plain --count 1)"
has 'NIXAIOS dash' "$out"
has 'energy' "$out"

echo "PASS: vampiro V0–V7 (host)"
