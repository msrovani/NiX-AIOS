#!/usr/bin/env bash
# Eco-Tokens v3 verify — stack=rust (NixAIOS: workspace em packages/nosd)
# smart_trim: se >20 linhas → 5 head + 15 tail; strip ANSI ECMA-48.
#
# Default (ordem de AGENTS.md, tudo no workspace packages/nosd):
#   cargo fmt --all --check
#   cargo clippy --workspace --all-targets --all-features -- -D warnings
#   cargo test --workspace
#
# Desvios por ambiente (o que NÃO corre fica explícito no log):
#   ECO_SKIP_CLIPPY=1   salta clippy
#   ECO_SKIP_TEST=1     salta testes
#   ECO_VERIFY_CMD=...  substitui os três passos por um comando próprio
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1
STACK="rust"
WORKSPACE="packages/nosd"
LOG="$(mktemp -t eco-verify.XXXXXX 2>/dev/null || mktemp)"
trap 'rm -f "$LOG"' EXIT

strip_ansi() { sed -E 's/\x1B(@|[\[\\]-_]|\[[0-?]*[ -/]*[@-~])//g'; }

smart_trim_fail() {
  local clean n head tail
  clean="$(strip_ansi <"$LOG" | tr -d '\r')"
  n="$(printf '%s\n' "$clean" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"
  if [[ "${n:-0}" -le 20 ]]; then
    printf '%s\n' "$clean" | sed '/^[[:space:]]*$/d' >&2
    return
  fi
  head="$(printf '%s\n' "$clean" | sed '/^[[:space:]]*$/d' | head -n 5)"
  tail="$(printf '%s\n' "$clean" | sed '/^[[:space:]]*$/d' | tail -n 15)"
  echo "----- smart_trim (5 head + 15 tail) -----" >&2
  printf '%s\n' "$head" >&2
  echo "…" >&2
  printf '%s\n' "$tail" >&2
}

# Git Bash: não deixar o link do Git sombrear o link.exe MSVC
case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*)
    CLEAN=""
    IFS=':'
    for p in $PATH; do
      case "$p" in */Git/usr/bin|*/Git/usr/bin/|*/git/usr/bin|*/git/usr/bin/) continue ;; esac
      CLEAN="${CLEAN:+$CLEAN:}$p"
    done
    unset IFS
    export PATH="$CLEAN"
    ;;
esac

run_step() {
  local label="$1"; shift
  echo "== $label" >"$LOG"
  "$@" >>"$LOG" 2>&1
}

rc=0
if [[ -n "${ECO_VERIFY_CMD:-}" ]]; then
  echo "== ECO_VERIFY_CMD" >"$LOG"
  ( eval "$ECO_VERIFY_CMD" ) >>"$LOG" 2>&1 || rc=$?
else
  if [[ ! -d "$ROOT/$WORKSPACE" ]]; then
    echo "VERIFY_FAIL stack=$STACK reason=missing_workspace:$WORKSPACE" >&2
    exit 1
  fi
  cd "$ROOT/$WORKSPACE" || exit 1
  run_step "cargo fmt --all --check" cargo fmt --all --check || rc=$?
  if [[ "$rc" -eq 0 && "${ECO_SKIP_CLIPPY:-0}" != "1" ]]; then
    run_step "cargo clippy -D warnings" \
      cargo clippy --workspace --all-targets --all-features -- -D warnings || rc=$?
  elif [[ "$rc" -eq 0 ]]; then
    echo "SKIP clippy (ECO_SKIP_CLIPPY=1)" >>"$LOG"
  fi
  if [[ "$rc" -eq 0 && "${ECO_SKIP_TEST:-0}" != "1" ]]; then
    run_step "cargo test --workspace" cargo test --workspace || rc=$?
  elif [[ "$rc" -eq 0 ]]; then
    echo "SKIP tests (ECO_SKIP_TEST=1)" >>"$LOG"
  fi
fi

if [[ "$rc" -eq 0 ]]; then
  # mostra os SKIP (se existirem) para a verificação não mentir sobre cobertura
  grep -E '^SKIP ' "$LOG" 2>/dev/null || true
  echo "VERIFY_OK stack=$STACK"
  exit 0
fi
echo "VERIFY_FAIL stack=$STACK exit=$rc" >&2
smart_trim_fail
exit 1
