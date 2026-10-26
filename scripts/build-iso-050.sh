#!/usr/bin/env bash
# Alias legado → scripts/build-iso.sh (NIXAIOS051).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
echo "build-iso-050.sh: redirect → build-iso.sh"
exec bash "$ROOT/scripts/build-iso.sh"
