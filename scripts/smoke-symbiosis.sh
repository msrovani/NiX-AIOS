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

echo "== S9 self-status (quem sou / o que posso / onde procurar) =="
out="$(run self)"
has '"identity"' "$out"
has '"NixAIOS"' "$out"
has '"capabilities"' "$out"
has '"entrails"' "$out"
has '"pointers"' "$out"
has '"methods": [0-9]{2,}' "$out"
has '"limits"' "$out"
# O autoconhecimento tem de listar a fonte de verdade e a saída para a web.
has 'net\.research' "$out"
has 'nix\.status' "$out"

echo "== S10 nix-audit (dívida de conhecimento) =="
out="$(run nix-audit)"
has '"total": [0-9]+' "$out"
has '"stale"' "$out"
has '"stale_lessons"' "$out"
has '"basis": "(observed|host_nao_nixos)"' "$out"

# S11: pesquisa web falha FECHADA quando não há rede/alvo (nunca sucesso fingido).
echo "== S11 research fail-closed (alvo inalcançável) =="
if out="$(run research "https://127.0.0.1:1/x" 2>&1)"; then
  fail "net.research devolveu ok para um alvo inalcançável"
else
  has 'net_unavailable|status' "$out"
fi
# Esquema não-https tem de ser recusado localmente, sem rede.
if out="$(run research "file:///etc/passwd" 2>&1)"; then
  fail "net.research aceitou file://"
else
  has 'bad_args|só https' "$out"
fi

echo "PASS: simbiose (nix.status/nix.ask/nix.audit/self.status/net.research/orb.symbiosis/factory.*)"
