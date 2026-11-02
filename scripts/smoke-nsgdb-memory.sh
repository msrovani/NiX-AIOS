#!/usr/bin/env bash
# F6: smoke governança memória (legacy backend — sem download git no CI host).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/packages/nosd"

export NOSD_MEMORY_BACKEND=nsgdb_legacy
# Lab: HITL soft só neste smoke (prod/ISO sem env = fail-closed).
export NOSD_HITL_LAB=1
cargo test -p nos-memory --lib -- --nocapture
cargo test -p nos-hermes cognitive_bridge -- --nocapture
cargo test -p nos-hermes wrap_opar -- --nocapture
cargo test -p nosd kiosk_methods_are_scope_limited -- --nocapture
cargo test -p nos-ipc --lib -- --nocapture

bash "$ROOT/scripts/nsgdb-pin-check.sh"
echo "smoke-nsgdb-memory PASS"
