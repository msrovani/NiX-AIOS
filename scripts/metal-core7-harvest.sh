#!/usr/bin/env bash
# Colheita metal Core 7 (ADR-0113: QEMU != metal).
#
# Dois criterios, provados no hardware e nao por substring (licao 30):
#   V1  uma so copia do 4B      -> exactamente 1 processo com -m ...4B...
#   V2  argv sem -ngl           -> 0 tokens '-ngl' em TODOS os motores
#   V3  o 4B esta de facto aberto -> 1 (e so um) processo com o GGUF em fd
#   V4  o fit decide            -> '--fit on' presente no argv do 4B
#
# Uso (no metal, root):
#   sudo bash scripts/metal-core7-harvest.sh [OUTDIR]
#   OUTDIR default: /mnt/nixaios-log/harvest/<utc> (FAT legivel no Windows)
#
# Verificacao de um bundle no host (sem o metal):
#   bash scripts/metal-core7-harvest.sh --verify docs/evidence/hw/<bundle>
#   -> imprime V1..V4 + PASS/FAIL e sai 1 se falhar.
#
# Nao promove nada a VERIFIED: empacota a prova, o julgamento fica no doc.
set -uo pipefail

GGUF_4B_HINT="${GGUF_4B_HINT:-4B}"

say() { printf '%s\n' "$*"; }
hdr() { printf '\n=== %s ===\n' "$*"; }

# ---------------------------------------------------------------- --verify
if [ "${1:-}" = "--verify" ]; then
  B="${2:-}"
  [ -n "$B" ] && [ -d "$B" ] || { say "uso: $0 --verify <bundle>"; exit 2; }
  T="$B/engines.tsv"
  [ -f "$T" ] || { say "FAIL: falta $T"; exit 2; }
  say "bundle: $B"
  say "--- engines.tsv ---"
  column -t -s $'\t' "$T" 2>/dev/null || cat "$T"

  n4b=0
  ngl_total=0
  ngl_bad=""
  n_engines=0
  while IFS=$'\t' read -r pid ppid model ngl fit ctx size; do
    [ -n "${pid:-}" ] || continue
    case "$pid" in \#*) continue ;; esac
    n_engines=$((n_engines + 1))
    ngl_total=$((ngl_total + ${ngl:-0}))
    case "$model" in
      *"$GGUF_4B_HINT"*) n4b=$((n4b + 1)) ;;
    esac
    if [ "${ngl:-0}" -gt 0 ]; then ngl_bad="$ngl_bad $pid($ngl)"; fi
  done <"$T"

  holders="$(grep -c '^HOLDER ' "$B/holders.txt" 2>/dev/null || printf 0)"
  case "$holders" in ''|*[!0-9]*) holders=0 ;; esac

  v1=fail; [ "$n4b" -eq 1 ] && v1=pass
  v2=fail; [ "$ngl_total" -eq 0 ] && v2=pass
  v3=fail; [ "$holders" -eq 1 ] && v3=pass
  v4=fail; grep -qE '^4B_FIT=on$' "$B/verdict.txt" 2>/dev/null && v4=pass

  say
  say "V1 copies_4b=${n4b} -> $v1"
  say "V2 ngl_tokens=${ngl_total}${ngl_bad:+ (offenders:$ngl_bad)} -> $v2"
  say "V3 holders_gguf=${holders} -> $v3"
  say "V4 fit_on_4b=${v4}"
  say "engines=${n_engines}"
  if [ "$v1$v2$v3$v4" = "passpasspasspass" ]; then
    say "RESULT=PASS"
    exit 0
  fi
  say "RESULT=FAIL"
  exit 1
fi

# ---------------------------------------------------------------- harvest
OUT="${1:-/mnt/nixaios-log/harvest/$(date -u +%Y%m%dT%H%M%SZ)}"
if ! mkdir -p "$OUT" 2>/dev/null || ! : >"$OUT/.write" 2>/dev/null; then
  OUT="/var/lib/nosd/harvest-$(date -u +%Y%m%dT%H%M%SZ)"
  mkdir -p "$OUT"
  say "WARN: FAT nao escrevivel; bundle em $OUT (copiar antes do poweroff)"
fi
rm -f "$OUT/.write"
exec > >(tee "$OUT/harvest.log") 2>&1

SINCE="$(date -Is)"
say "=== Core 7 harvest $(date -Is) ==="
say "host=$(cat /etc/hostname 2>/dev/null) kernel=$(uname -r) OUT=$OUT"

hdr "units"
for u in nosd jarbas-cage nosd-engine-s1 nosd-engine-s2 seatd; do
  printf '%-16s %s\n' "$u" "$(systemctl is-active "$u" 2>/dev/null || true)"
done | tee "$OUT/units.txt"
systemctl show nosd-engine-s2 -p MainPID -p SubState -p ExecStart --no-pager \
  >"$OUT/unit-s2.txt" 2>&1 || true

hdr "modelos presentes"
ls -l /var/lib/nosd/models/*.gguf 2>/dev/null | tee "$OUT/models.txt" || say "sem GGUFs"

# --- motores: todo o processo cujo argv traz llama-server/llama-cli ------
hdr "engines (argv real de /proc/<pid>/cmdline)"
ENGINES_TSV="$OUT/engines.tsv"
: >"$ENGINES_TSV"
printf '#pid\tppid\tmodel\tngl\tfit\tctx\tsize\n' >>"$ENGINES_TSV"

PIDS="$(pgrep -f 'llama-server|llama-cli' 2>/dev/null | sort -n || true)"
if [ -z "$PIDS" ]; then
  say "nenhum llama-server/llama-cli a correr (EngineAbsent nao e falha de argv)"
fi

for pid in $PIDS; do
  [ -r "/proc/$pid/cmdline" ] || continue
  mapfile -d '' TOK <"/proc/$pid/cmdline" 2>/dev/null || continue
  [ "${#TOK[@]}" -gt 0 ] || continue

  printf '%s\n' "${TOK[@]}" >"$OUT/engine-$pid.argv"

  model=""; ngl=0; fit=none; ctx=none
  i=0
  while [ "$i" -lt "${#TOK[@]}" ]; do
    case "${TOK[$i]}" in
      -ngl) ngl=$((ngl + 1)) ;;
      -m) model="${TOK[$((i + 1))]:-}" ;;
      --model) model="${TOK[$((i + 1))]:-}" ;;
      --fit) fit="${TOK[$((i + 1))]:-on}" ;;
      -c) ctx="${TOK[$((i + 1))]:-}" ;;
    esac
    i=$((i + 1))
  done
  # contar -ngl como TOKEN exacto, nunca substring (licao 30): "-ngl99" nao existe
  ngl_extra="$(grep -cx -- '-ngl' "$OUT/engine-$pid.argv" 2>/dev/null || printf 0)"
  case "$ngl_extra" in ''|*[!0-9]*) ngl_extra=0 ;; esac

  ppid="$(awk '{print $4}' "/proc/$pid/stat" 2>/dev/null || echo '?')"
  size=0
  [ -n "$model" ] && [ -f "$model" ] && size="$(stat -c %s "$model" 2>/dev/null || echo 0)"

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$pid" "$ppid" "${model:-?}" "$ngl_extra" "$fit" "${ctx:-auto}" "$size" >>"$ENGINES_TSV"
  say "pid=$pid model=${model:-?} ngl=$ngl_extra fit=$fit ctx=${ctx:-auto} size=$size"
done

# --- quem tem o GGUF aberto (a prova de "uma so copia" carregada) --------
hdr "holders do GGUF do 4B"
GGUF4B=""
while IFS=$'\t' read -r pid ppid model ngl fit ctx size; do
  case "${pid:-}" in ''|\#*) continue ;; esac
  case "$model" in
    *"$GGUF_4B_HINT"*) GGUF4B="$model"; break ;;
  esac
done <"$ENGINES_TSV"
if [ -z "$GGUF4B" ]; then
  # sem 4B no argv: usar o maior GGUF presente como referencia honesta
  GGUF4B="$(ls -S /var/lib/nosd/models/*.gguf 2>/dev/null | head -1 || true)"
  [ -n "$GGUF4B" ] && say "WARN: nenhum motor com '*${GGUF_4B_HINT}*' no argv; referencia=$(basename "$GGUF4B")"
fi
: >"$OUT/holders.txt"
if [ -n "$GGUF4B" ]; then
  REAL="$(readlink -f "$GGUF4B" 2>/dev/null || printf '%s' "$GGUF4B")"
  for p in /proc/[0-9]*; do
    pid="${p#/proc/}"
    [ -d "$p/fd" ] || continue
    for fd in "$p"/fd/*; do
      tgt="$(readlink "$fd" 2>/dev/null || true)"
      if [ -n "$tgt" ] && [ "$tgt" = "$REAL" ]; then
        printf 'HOLDER %s %s\n' "$pid" "$(cat "$p/comm" 2>/dev/null)" | tee -a "$OUT/holders.txt"
        break
      fi
    done
  done
  say "gguf=$GGUF4B"
  ls -l "$REAL" 2>/dev/null | tee -a "$OUT/holders.txt" || true
else
  say "sem GGUF para inspeccionar"
fi

# --- GPU / CUDA (contexto, nao criterio) ---------------------------------
hdr "GPU"
NVIDIA_SMI="${NOSD_NVIDIA_SMI:-nvidia-smi}"
if command -v "$NVIDIA_SMI" >/dev/null 2>&1; then
  "$NVIDIA_SMI" --query-gpu=name,driver_version,memory.total,memory.used \
    --format=csv,noheader >"$OUT/nvidia-smi.txt" 2>&1 || true
  "$NVIDIA_SMI" --query-compute-apps=pid,process_name,used_memory \
    --format=csv,noheader >"$OUT/nvidia-compute-apps.txt" 2>&1 || true
  cat "$OUT/nvidia-smi.txt" "$OUT/nvidia-compute-apps.txt"
else
  say "nvidia-smi ausente ($NVIDIA_SMI) — EngineAbsent/CPU e honestidade, nao falha"
  : >"$OUT/nvidia-smi.txt"
fi

# --- marcadores que so o codigo novo emite (licao 66) --------------------
hdr "marcadores no journal"
journalctl -b -u nosd -u nosd-engine-s2 -u nosd-engine-s1 -u jarbas-cage -u jarbas-hub \
  --no-pager >"$OUT/journal.txt" 2>&1 || true
grep -aE 'suggest_ngl=|spawned .*ngl=|hub slot=|transmute offload hint' "$OUT/journal.txt" \
  >"$OUT/markers.txt" 2>/dev/null || true
cat "$OUT/markers.txt" 2>/dev/null || say "sem marcadores"

# --- criterios -----------------------------------------------------------
COPIES_4B=0
NGL_TOTAL=0
FIT_4B=off
while IFS=$'\t' read -r pid ppid model ngl fit ctx size; do
  case "${pid:-}" in ''|\#*) continue ;; esac
  NGL_TOTAL=$((NGL_TOTAL + ${ngl:-0}))
  case "$model" in
    *"$GGUF_4B_HINT"*)
      COPIES_4B=$((COPIES_4B + 1))
      FIT_4B="${fit:-none}"
      ;;
  esac
done <"$ENGINES_TSV"
HOLDERS="$(grep -c '^HOLDER ' "$OUT/holders.txt" 2>/dev/null || printf 0)"
case "$HOLDERS" in ''|*[!0-9]*) HOLDERS=0 ;; esac

{
  printf '4B_PROCESSES=%s\n' "$COPIES_4B"
  printf 'NGL_TOKENS=%s\n' "$NGL_TOTAL"
  printf 'GGUF_HOLDERS=%s\n' "$HOLDERS"
  printf '4B_FIT=%s\n' "$FIT_4B"
} >"$OUT/verdict.txt"
cp "$ENGINES_TSV" "$OUT/engines.tsv" 2>/dev/null || true

hdr "veredicto"
cat "$OUT/verdict.txt"
say "bundle=$OUT"
say "=== DONE ==="
say "No host: bash scripts/metal-core7-harvest.sh --verify <bundle>"
