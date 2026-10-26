#!/usr/bin/env bash
# Host smoke: resolve_models_dir Linux-first (0.5.0) — sem FAT.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/packages/nosd"
unset NIXAIOS_USE_FAT_PERSIST || true
cargo test -p nosd resolve_models_dir_linux_first -- --nocapture
cargo test -p nosd landlock -- --nocapture
cargo test -p nos-mesh crdt -- --nocapture
cargo test -p nos-mesh soul_mirror -- --nocapture
echo "PASS: smoke-transmute-paths (Linux-first + landlock FAT allow + CRDT/SoulMirror)"
