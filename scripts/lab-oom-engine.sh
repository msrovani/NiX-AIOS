#!/usr/bin/env bash
# W2 lab: stress MemoryMax no nosd-engine@ — cage deve permanecer active.
# Uso (metal/NixAIOS): sudo bash lab-oom-engine.sh [port=8081]
set -euo pipefail
PORT="${1:-8081}"
UNIT="nosd-engine@${PORT}.service"
echo "=== lab OOM engine $UNIT ==="
systemctl is-active jarbas-cage.service || echo "WARN: cage not active (start UI first)"
systemctl cat "$UNIT" | grep -E 'OOMScoreAdjust|MemoryMax|Device' || true
# Limiar temporário baixo para forçar OOM do motor (restaura no fim)
systemctl set-property "$UNIT" MemoryMax=64M || true
systemctl restart "$UNIT" || true
sleep 2
# Alloc hostil via llama se modelo existir; senão stressdd no cgroup
CG=$(systemctl show -p ControlGroup --value "$UNIT" || true)
echo "cgroup=$CG"
if [[ -n "$CG" && -d "/sys/fs/cgroup$CG" ]]; then
  echo "stress: writing until OOM in engine cgroup (best-effort)"
  timeout 15s dd if=/dev/zero of=/dev/null bs=64M count=64 2>/dev/null &
  # Preferir processo no cgroup do unit
  systemd-run --unit=nosd-engine-stress --property="MemoryMax=64M" \
    --property="Slice=$(systemctl show -p Slice --value "$UNIT" 2>/dev/null || echo system.slice)" \
    dd if=/dev/zero of=/tmp/nosd-engine-oom.bin bs=1M count=128 || true
fi
sleep 3
echo "cage=$(systemctl is-active jarbas-cage.service || true)"
echo "engine=$(systemctl is-active "$UNIT" || true)"
journalctl -u "$UNIT" -u jarbas-cage.service -n 30 --no-pager || true
systemctl set-property "$UNIT" MemoryMax=24G || true
echo "=== expect: cage=active; engine=failed/oom (not cage) ==="
