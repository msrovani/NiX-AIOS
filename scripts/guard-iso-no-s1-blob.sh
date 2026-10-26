#!/usr/bin/env bash
# W3 / S1.2 REJECT: ISO não embute AgentJev / Piper seed / GGUF S1.
# Honesty: NÃO rodar `strings` no ISO inteiro (2.7G+ = hang). Checagens leves.
set -euo pipefail
TARGET="${1:?iso or directory}"

fail() { echo "FAIL: $*"; exit 1; }

# 1) Ficheiros GGUF/AgentJev ao lado do artefacto
DIR=$(dirname "$TARGET")
if find "$DIR" -maxdepth 2 \( -iname '*agentjev*.gguf' -o -iname 'agentjev*.bin' \) 2>/dev/null | grep -q .; then
  fail "agentjev blob next to artifact under $DIR"
fi

if [[ -d "$TARGET" ]]; then
  if find "$TARGET" \( -iname '*.gguf' -o -iname '*agentjev*' \) 2>/dev/null | grep -q .; then
    fail "gguf/agentjev inside directory tree $TARGET"
  fi
  echo "OK: directory tree clean of S1 blobs ($TARGET)"
  exit 0
fi

[[ -f "$TARGET" ]] || fail "not a file: $TARGET"

# 2) Amostra dos primeiros 8MiB (PVD/labels) — sem varrer 2.7G
SAMPLE=$(head -c 8388608 "$TARGET" | tr -cd '[:print:]\n' | grep -Eai 'AgentJev|agentjev\.gguf' || true)
if [[ -n "$SAMPLE" ]]; then
  fail "S1 fingerprint in ISO header sample"
fi

# 3) Política documentada no flake (comentário iso.nix) — aviso se volid ok
echo "OK: no AgentJev/S1 seed near/header of $TARGET (full-ISO strings skipped by design)"
