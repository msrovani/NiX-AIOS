#!/usr/bin/env bash
# Gate objectivo Boot→JARBAS (docs/BOOT_TO_JARBAS.md) — stdout = ground truth IA.
# claim_status: OBSERVED|IMPLEMENTED|AWAITING_HW|UNKNOWN|BLOCKED (ADR-0113).
#
# O-01/G5.1: `--strict` transforma SKIP em estágio OBRIGATÓRIO em FAIL
# (a cultura "SKIP deixa de ser ok, não aplicável" do IMPROVEMENT_PLAN §9).
# Estágios obrigatórios configuráveis via NIXAIOS_GATE_REQUIRED (default = o
# aceite 0.6.x: substrato + M3 + MQEMU). M4/M5 ficam fora por serem
# AWAITING_HW honestos (hardware-gated, não capability skip).
set -euo pipefail

STAGE="ALL"
STRICT=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --stage) STAGE="${2:-ALL}"; shift 2 ;;
    --strict) STRICT=1; shift ;;
    *) STAGE="$1"; shift ;;
  esac
done

TCP="${NOSD_TCP:-127.0.0.1:7700}"
DATA="${NOSD_DATA_DIR:-/var/lib/nosd}"
# S-01: TCP exige token; Unix socket usa SO_PEERCRED.
TOKEN_FILE="${NOSD_IPC_TOKEN_FILE:-$DATA/ipc.token}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Prefer Unix socket canónico (alinhar nosd/jarbas); TCP só se socket ausente.
if [[ -n "${NOSD_SOCKET:-}" ]]; then
  SOCK="$NOSD_SOCKET"
elif [[ -S /run/nosd/nosd.sock ]]; then
  SOCK="/run/nosd/nosd.sock"
else
  SOCK=""
fi
FAILS=0
# G5.1: estágios obrigatórios no modo strict (SKIP aí = FAIL, não "não aplicável").
REQUIRED_STAGES="${NIXAIOS_GATE_REQUIRED:-M0 M1 M2 M3b M3c M3d M3f MQEMU}"

is_required_stage() {
  local s
  for s in $REQUIRED_STAGES; do
    [[ "$s" == "$1" ]] && return 0
  done
  return 1
}

nosctl_bin() {
  if command -v nosctl >/dev/null 2>&1; then
    command -v nosctl
  elif [[ -x "$REPO_ROOT/packages/nosd/target/debug/nosctl" ]]; then
    echo "$REPO_ROOT/packages/nosd/target/debug/nosctl"
  elif [[ -x "$REPO_ROOT/packages/nosd/target/release/nosctl" ]]; then
    echo "$REPO_ROOT/packages/nosd/target/release/nosctl"
  else
    echo ""
  fi
}

NOSCTL="$(nosctl_bin)"

rpc() {
  local method="$1"
  shift || true
  if [[ -z "$NOSCTL" ]]; then
    echo "BLOCKED nosctl_missing"
    return 2
  fi
  if [[ -n "$SOCK" && -S "$SOCK" ]]; then
    "$NOSCTL" --socket "$SOCK" --token-file "$TOKEN_FILE" "$method" "$@" 2>/dev/null || return 1
  else
    "$NOSCTL" --tcp "$TCP" --token-file "$TOKEN_FILE" "$method" "$@" 2>/dev/null || return 1
  fi
}

emit() {
  # stage ring layer claim result detail
  if [[ "$STRICT" == 1 && "$5" == "SKIP" ]] && is_required_stage "$1"; then
    FAILS=$((FAILS + 1))
    printf 'stage=%s ring=%s layer=%s claim_status=%s result=FAIL detail=strict_skip: %s\n' \
      "$1" "$2" "$3" "$4" "$6"
    return
  fi
  printf 'stage=%s ring=%s layer=%s claim_status=%s result=%s detail=%s\n' \
    "$1" "$2" "$3" "$4" "$5" "$6"
}

gate_fail() {
  FAILS=$((FAILS + 1))
  emit "$1" "$2" "$3" "$4" "FAIL" "$5"
}

gate_pass() {
  emit "$1" "$2" "$3" "$4" "PASS" "$5"
}

want() {
  [[ "$STAGE" == "ALL" || "$STAGE" == "$1" || "$STAGE" == "M${1#M}" ]]
}

# --- M0 ---
if want M0; then
  if out="$(rpc health 2>/dev/null)"; then
    if echo "$out" | grep -qiE '"status"[[:space:]]*:[[:space:]]*"ok"|status.:.ok'; then
      gate_pass M0 Ring3_Linux "PORT+ADAPT+DELEGATE" OBSERVED "health.status=ok"
    else
      gate_fail M0 Ring3_Linux "PORT+ADAPT+DELEGATE" UNKNOWN "health_no_ok"
    fi
  else
    # Host sem daemon = UNKNOWN (não FAIL de produto metal)
    gate_fail M0 Ring3_Linux "PORT+ADAPT+DELEGATE" UNKNOWN "nosd_unreachable tcp=$TCP sock=${SOCK:-none}"
  fi
  if [[ -d "$DATA/nsgdb" ]] || [[ -d "./data/nsgdb" ]]; then
    gate_pass M0 Ring3_Linux PORT IMPLEMENTED "nsgdb_dir_present"
  else
    emit M0 Ring3_Linux PORT UNKNOWN SKIP "nsgdb_dir_absent (ok se data-dir diferente)"
  fi
fi

# --- M1 ---
if want M1; then
  if [[ -f "$DATA/boot-score.json" ]]; then
    if python3 -c "import json;json.load(open('$DATA/boot-score.json'))" 2>/dev/null; then
      gate_pass M1 Ring3_Linux ADAPT OBSERVED "boot-score.json_parseable"
    else
      gate_fail M1 Ring3_Linux ADAPT UNKNOWN "boot-score_unparseable"
    fi
  else
    emit M1 Ring3_Linux ADAPT AWAITING_HW SKIP "boot-score.json absent (host/WSL sem early unit)"
  fi
  if out="$(rpc score 2>/dev/null || rpc health 2>/dev/null)"; then
    if echo "$out" | grep -qiE 'keyword|scorer_absent|s1_backend|landlock'; then
      gate_pass M1 Ring3_Linux PORT OBSERVED "s1_or_score_fields_present"
    else
      gate_pass M1 Ring3_Linux PORT OBSERVED "score_or_health_rpc_ok"
    fi
  fi
fi

# --- M2 ---
if want M2; then
  if [[ -f "$DATA/firstboot.done" ]]; then
    gate_pass M2 Ring3_Linux DELEGATE OBSERVED "firstboot.done"
  elif [[ -f "$DATA/firstboot.partial" ]] || [[ -f "$DATA/firstboot.step" ]]; then
    gate_pass M2 Ring3_Linux DELEGATE OBSERVED "firstboot_partial_or_step"
  else
    emit M2 Ring3_Linux DELEGATE AWAITING_HW SKIP "no firstboot markers (expected on host TCP lab)"
  fi
fi

# --- M3a Trinity / route ---
if want M3a || want M3; then
  # Prefer nosctl `route` → RPC route.explain (trinity + Decision 0106 + S1 LOO)
  if out="$(rpc route oi 2>/dev/null)"; then
    if echo "$out" | grep -qiE 'trinity|score_v0|decision|scorer_absent|blob_deliberate'; then
      gate_pass M3a Ring3_Linux "PORT+ADAPT" IMPLEMENTED "route.explain_trinity_decision"
    elif echo "$out" | grep -qiE 'slot|lane|scores'; then
      gate_pass M3a Ring3_Linux "PORT+ADAPT" OBSERVED "route.explain_basic"
    else
      gate_pass M3a Ring3_Linux "PORT+ADAPT" OBSERVED "route_rpc_ok"
    fi
  elif out_h="$(rpc health 2>/dev/null)"; then
    if echo "$out_h" | grep -qiE 'hub_loaded|engine|s1_backend'; then
      gate_pass M3a Ring3_Linux "PORT+ADAPT" OBSERVED "health_hub_engine_s1"
    else
      gate_pass M3a Ring3_Linux "PORT+ADAPT" OBSERVED "health_ok_trinity_fields_soft"
    fi
  else
    gate_fail M3a Ring3_Linux "PORT+ADAPT" UNKNOWN "health_unreachable"
  fi
fi

# --- M3b cage + chat/cpu-echo honesty ---
if want M3b || want M3; then
  if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet jarbas-cage.service 2>/dev/null; then
    gate_pass M3b Ring3_Linux ADAPT OBSERVED "jarbas-cage_active"
  else
    emit M3b Ring3_Linux ADAPT AWAITING_HW SKIP "jarbas-cage not active (host lab / QEMU OK)"
  fi
  if out="$(rpc health 2>/dev/null)"; then
    if echo "$out" | grep -qiE 'cpu-echo|llama|hub_loaded|engine'; then
      gate_pass M3b Ring3_Linux "PORT+ADAPT" OBSERVED "engine_or_cpu-echo_documented"
    fi
  fi
  if out="$(rpc ui-card 'Boot HITL' --kv 'stage=M3b,adr=0058' 2>/dev/null)"; then
    if echo "$out" | grep -qiE 'window|card|0058'; then
      gate_pass M3b Ring3_Linux PORT IMPLEMENTED "ui.card_hitl_thin"
    fi
  else
    emit M3b Ring3_Linux PORT IMPLEMENTED SKIP "ui.card unavailable (nosd old or down)"
  fi
fi

# --- M3c ticks ---
if want M3c || want M3; then
  if out="$(rpc health 2>/dev/null)"; then
    ticks=$(echo "$out" | python3 -c "import sys,json,re; t=sys.stdin.read();
import json as J
try:
  j=J.loads(t); r=j.get('result') or j
  print(int(r.get('hermes_ticks') or 0))
except Exception:
  m=re.search(r'hermes_ticks[^0-9]*([0-9]+)', t); print(int(m.group(1)) if m else 0)" 2>/dev/null || echo 0)
    if [[ "${ticks:-0}" -gt 0 ]]; then
      gate_pass M3c Ring3_Linux PORT OBSERVED "hermes_ticks=$ticks"
    else
      emit M3c Ring3_Linux PORT UNKNOWN SKIP "hermes_ticks=0 (continuous off or just started)"
    fi
  fi
fi

# --- M3d Frugal ---
if want M3d || want M3; then
  if [[ -x "$REPO_ROOT/scripts/smoke-frugal-fleet.sh" ]]; then
    # env -u ROOT: smoke usa NOSD_CRATE_ROOT; não herdar REPO_ROOT como ROOT
    if env -u ROOT NOSD_CRATE_ROOT="$REPO_ROOT/packages/nosd" \
      bash "$REPO_ROOT/scripts/smoke-frugal-fleet.sh" >/tmp/gate-frugal.out 2>&1; then
      gate_pass M3d Ring3_Linux PORT OBSERVED "smoke-frugal-fleet_PASS"
    else
      gate_fail M3d Ring3_Linux PORT UNKNOWN "smoke-frugal-fleet_FAIL"
    fi
  else
    emit M3d Ring3_Linux PORT UNKNOWN SKIP "smoke-frugal-fleet.sh missing"
  fi
fi

# --- M3f voice ---
if want M3f || want M3; then
  # nosctl voice-status — exigir posture Ready|EngineAbsent (não grep "mic" sempre presente)
  if out="$(rpc voice-status 2>/dev/null)"; then
    if echo "$out" | grep -qiE '"posture"[[:space:]]*:[[:space:]]*"(Ready|EngineAbsent|EngineReady)'; then
      gate_pass M3f Ring3_Linux ADAPT OBSERVED "voice.posture_Ready_or_Absent"
    elif echo "$out" | grep -qiE 'EngineAbsent|EngineReady|"stt_ready"'; then
      gate_pass M3f Ring3_Linux ADAPT OBSERVED "voice.status_engine_fields"
    else
      emit M3f Ring3_Linux ADAPT UNKNOWN SKIP "voice-status sem posture honesty"
    fi
  else
    emit M3f Ring3_Linux ADAPT AWAITING_HW SKIP "voice-status unreachable"
  fi
fi

# --- M4 metal markers ---
if want M4; then
  if [[ -f "$DATA/firstboot.done" ]] && command -v systemctl >/dev/null 2>&1 \
    && systemctl is-active --quiet jarbas-cage.service 2>/dev/null \
    && systemctl is-active --quiet nosd.service 2>/dev/null; then
    gate_pass M4 Ring3_Linux_metal "ADAPT+DELEGATE" OBSERVED "firstboot.done+cage+nosd (attach journal for VERIFIED)"
  else
    emit M4 Ring3_Linux_metal DELEGATE AWAITING_HW SKIP "metal campaign — see docs/METAL_CAMPAIGN.md"
  fi
fi

# --- M5 fleet ---
if want M5; then
  if out="$(rpc health 2>/dev/null)"; then
    peers=$(echo "$out" | python3 -c "import sys,json,re; t=sys.stdin.read();
import json as J
try:
  j=J.loads(t); r=j.get('result') or j
  print(int(r.get('mesh_peers') or 0))
except Exception:
  m=re.search(r'mesh_peers[^0-9]*([0-9]+)', t); print(int(m.group(1)) if m else 0)" 2>/dev/null || echo 0)
    if [[ "${peers:-0}" -ge 1 ]]; then
      gate_pass M5 Ring3_Linux PORT OBSERVED "mesh_peers=$peers"
    else
      emit M5 Ring3_Linux PORT AWAITING_HW SKIP "mesh_peers=0 — docs/FLEET_LAB.md (0081 A/B)"
    fi
  fi
fi

# --- MQEMU — lab ISO (≠ metal VERIFIED) ---
if want MQEMU || want QEMU; then
  if [[ -n "${NIXAIOS_QEMU_LOG:-}" && -f "${NIXAIOS_QEMU_LOG}" ]]; then
    if grep -qiE 'NixOS|Linux version|Welcome|systemd|Reached target' "$NIXAIOS_QEMU_LOG"; then
      gate_pass MQEMU Ring3_Linux_qemu DELEGATE OBSERVED "qemu_serial_boot_markers log=$NIXAIOS_QEMU_LOG"
    else
      gate_fail MQEMU Ring3_Linux_qemu DELEGATE UNKNOWN "qemu_log_no_boot_markers"
    fi
    # Honesty: boot markers ≠ nosd healthy
    if grep -qiE 'Failed to start .*Neural-OS Daemon|Failed to start .*nosd' "$NIXAIOS_QEMU_LOG"; then
      gate_fail MQEMU Ring3_Linux_qemu ADAPT OBSERVED "nosd.service_FAILED_in_qemu_log (≠ metal; diagnose guest)"
    elif grep -qiE 'Started .*Neural-OS Daemon|nosd\.service:.*Succeeded' "$NIXAIOS_QEMU_LOG"; then
      gate_pass MQEMU Ring3_Linux_qemu ADAPT OBSERVED "nosd_start_ok_in_qemu_log"
    else
      emit MQEMU Ring3_Linux_qemu ADAPT UNKNOWN SKIP "nosd start status unclear in qemu log"
    fi
  else
    emit MQEMU Ring3_Linux_qemu DELEGATE AWAITING_HW SKIP "set NIXAIOS_QEMU_LOG=/tmp/nixaios-qemu.log after run-iso-qemu-*.sh (≠ metal)"
  fi
fi

echo "---"
echo "gate_summary fails=$FAILS stage=$STAGE strict=$STRICT idea=#707 playbook=docs/BOOT_TO_JARBAS.md"
if [[ "$FAILS" -gt 0 ]]; then
  exit 1
fi
exit 0
