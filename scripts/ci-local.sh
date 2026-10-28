#!/usr/bin/env bash
# Espelho local do .github/workflows/ci.yml (mesmos passos, mesma ordem).
# Uso: bash scripts/ci-local.sh [--fast]
#   --fast  salta nix/VM (so fmt+clippy+test)
set -euo pipefail
cd "$(dirname "$0")/.."
FAST="${1:-}"
FEATURES=(--option system-features "kvm nixos-test big-parallel benchmark")

echo "== lint: fmt =="
( cd packages/nosd && cargo fmt --all --check )
echo "== lint: clippy (all features) =="
( cd packages/nosd && cargo clippy --workspace --all-targets --all-features -- -D warnings )
echo "== test =="
( cd packages/nosd && cargo test --workspace )

if [ "$FAST" = "--fast" ]; then
  echo "CI-local (fast) OK"
  exit 0
fi

echo "== nix: pacote nosd (compila + corre testes em Linux) =="
nix build .#nosd --print-out-paths

echo "== nix: VM smoke =="
nix build .#checks.x86_64-linux.nixaios-vm -L --no-link "${FEATURES[@]}"

# Um check de cada vez: 5 testes x 2 VMs em paralelo faz OOM em maquinas de 7 GB.
echo "== fleet (sequencial; --max-jobs 1) =="
for c in mesh-2vm mesh-2vm-signed mesh-2vm-jobs mesh-2vm-crdt mesh-2vm-skills mesh-3vm-faults; do
  echo "-- $c"
  nix build ".#checks.x86_64-linux.$c" -L --no-link --max-jobs 1 "${FEATURES[@]}"
done

echo "CI-local OK (GitHub ainda precisa da primeira execucao)"
