#!/usr/bin/env bash
# F0c: falha se pin docs ≠ flake rev / doctrine / SOUL / FileStorage leak.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VER="$ROOT/docs/NSGDB_VERSION.md"
TOML="$ROOT/packages/nosd/crates/nos-memory/Cargo.toml"
DOC="$ROOT/packages/nosd/crates/nos-memory/doctrine/DOCTRINE.md"

rev="$(grep -E '^\s*nsgdb_rev:' "$VER" | awk '{print $2}' | tr -d '\r')"
crate_ver="$(grep -E '^\s*nsgdb_crate_version:' "$VER" | awk '{print $2}' | tr -d '\r')"
flake_rev="$(grep -E 'neural-sgdb\.url' "$ROOT/flake.nix" | sed -E 's|.*/([a-f0-9]{7,40}).*|\1|' | head -1 | tr -d '\r')"

if [[ -z "$rev" ]]; then
  echo "pin-check: missing nsgdb_rev in NSGDB_VERSION.md" >&2
  exit 1
fi
if [[ -n "$flake_rev" ]]; then
  if [[ "${rev:0:7}" != "${flake_rev:0:7}" ]]; then
    echo "pin-check: rev mismatch docs=$rev flake=$flake_rev" >&2
    exit 1
  fi
fi

if ! grep -q "neural-sgdb" "$TOML"; then
  echo "pin-check: neural-sgdb dep missing in Cargo.toml" >&2
  exit 1
fi
if ! grep -qE "${crate_ver}|1\\.5\\.0|neural-sgdb" "$VER"; then
  echo "pin-check: version doc incomplete" >&2
  exit 1
fi
VENDOR="$ROOT/packages/nosd/vendor/neural-sgdb"
if [[ -f "$VENDOR/Cargo.toml" ]]; then
  vver="$(grep -E '^version' "$VENDOR/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/' | tr -d '\r')"
  if [[ -n "$vver" && -n "$crate_ver" && "$vver" != "$crate_ver" ]]; then
    echo "pin-check: vendor version $vver != docs $crate_ver" >&2
    exit 1
  fi
fi

if [[ ! -f "$DOC" ]]; then
  echo "pin-check: missing doctrine seed $DOC" >&2
  exit 1
fi
if ! grep -q "Null-scoping" "$DOC"; then
  echo "pin-check: doctrine seed missing Null-scoping" >&2
  exit 1
fi

SOUL="$ROOT/packages/nosd/crates/nos-hermes/soul/SOUL.md"
for m in NSGDB_DOCTRINE_V1 TWO_PASS NULL_SCOPE ADD_ONLY HIT_NOT_ORDER; do
  if ! grep -q "$m" "$SOUL"; then
    echo "pin-check: SOUL missing marker $m" >&2
    exit 1
  fi
done

# Grep CI: FileStorage fora de nos-memory = fail
leaks="$(grep -R --include='*.rs' -n 'FileStorage::' "$ROOT/packages/nosd/crates" 2>/dev/null | grep -v 'nos-memory' | grep -v '^Binary' || true)"
if [[ -n "$leaks" ]]; then
  echo "pin-check: FileStorage:: used outside nos-memory" >&2
  echo "$leaks" >&2
  exit 1
fi

echo "nsgdb-pin-check PASS rev=$rev doctrine_ok soul_ok"
