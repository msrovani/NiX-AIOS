#!/usr/bin/env bash
# Colheita metal Core 7 (ADR-0113: QEMU != metal).
#
# Criterios, provados no hardware e por campo, nunca por substring (licao 30):
#   V1  uma so copia do 4B       -> exactamente 1 processo com -m ...4B...
#   V2  argv sem -ngl            -> 0 tokens '-ngl' em TODOS os motores
#   V3  o 4B esta de facto aberto -> 1 (e so um) processo com o GGUF em fd
#   V4  o fit decide             -> '--fit on' presente no argv do 4B
#   V5  dispositivo por motor    -> cada motor que declara CUDA_VISIBLE_DEVICES
#                                   tem /dev/nvidiaN aberto (e vice-versa);
#                                   DISTINCT_GPUS e REPORTADO, nao exigido: na
#                                   hw-cuda.nix S1 e S2 usam a mesma dGPU (0).
#   V6  contexto escolhido       -> /props real de cada motor: '--fit on', sem
#                                   '-c' fixo, n_ctx>0 e >= minimo do --fit-ctx
#                                   (llama.cpp 11429: --fit-ctx e MINIMO, nao teto)
#
# Uso (no metal, root):
#   sudo bash scripts/metal-core7-harvest.sh [OUTDIR]
#   OUTDIR default: /mnt/nixaios-log/harvest/<utc> (FAT legivel no Windows)
#
# Verificacao de um bundle no host (sem o metal):
#   bash scripts/metal-core7-harvest.sh --verify docs/evidence/hw/<bundle>
#   -> imprime V1..V6 + PASS/FAIL/N/A; sai 1 se falhar, 2 se nao aplicavel.
#
# Nao promove nada a VERIFIED: empacota a prova, o julgamento fica no doc.
set -uo pipefail

GGUF_4B_HINT="${GGUF_4B_HINT:-4B}"

say() { printf '%s\n' "$*"; }
hdr() { printf '\n=== %s ===\n' "$*"; }

# valor de uma chave unica do verdict.txt (nunca substring: licao 30)
vkey() { sed -n "s/^$1=//p" "$2" 2>/dev/null | head -1; }

# GET /props em HTTP/1.0 (fecha no fim) sem curl nem python: bash /dev/tcp.
# $1=porta  $2=bearer (opcional, so S1 managed)  -> corpo em stdout
http_props() {
  local port="$1" key="${2:-}"
  { exec 3<>"/dev/tcp/127.0.0.1/$port"; } 2>/dev/null || return 1
  if [ -n "$key" ]; then
    printf 'GET /props HTTP/1.0\r\nHost: 127.0.0.1\r\nAuthorization: Bearer %s\r\n\r\n' "$key" >&3
  else
    printf 'GET /props HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n' >&3
  fi
  timeout 8 cat <&3 2>/dev/null || true
  exec 3<&- 3>&- 2>/dev/null || true
}

# ---------------------------------------------------------------- --verify
if [ "${1:-}" = "--verify" ]; then
  B="${2:-}"
  [ -n "$B" ] && [ -d "$B" ] || { say "uso: $0 --verify <bundle>"; exit 2; }
  T="$B/engines.tsv"
  V="$B/verdict.txt"
  [ -f "$T" ] || { say "FAIL: falta $T"; exit 2; }
  [ -f "$V" ] || { say "FAIL: falta $V"; exit 2; }
  say "bundle: $B"
  say "--- engines.tsv ---"
  column -t -s $'\t' "$T" 2>/dev/null || cat "$T"
  if [ -f "$B/devices.tsv" ]; then
    say "--- devices.tsv ---"
    column -t -s $'\t' "$B/devices.tsv" 2>/dev/null || cat "$B/devices.tsv"
  fi
  if [ -f "$B/fit-props.tsv" ]; then
    say "--- fit-props.tsv ---"
    column -t -s $'\t' "$B/fit-props.tsv" 2>/dev/null || cat "$B/fit-props.tsv"
  fi

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

  say
  say "VERDICT (bundle)"
  sed -n 'p' "$V"
  say
  if [ "$n_engines" -eq 0 ]; then
    say "V1..V6 na (EngineAbsent: nenhum motor no argv) -> RESULT=N/A"
    exit 2
  fi

  v1=fail; [ "$n4b" -eq 1 ] && v1=pass
  v2=fail; [ "$ngl_total" -eq 0 ] && v2=pass
  v3=fail; [ "$holders" -eq 1 ] && v3=pass
  v4=fail; [ "$(vkey 4B_FIT "$V")" = "on" ] && v4=pass
  v5="$(vkey DEVICE_MATCH "$V")"; [ -n "$v5" ] || v5=fail
  v6="$(vkey FIT_OK "$V")"; [ -n "$v6" ] || v6=fail

  say "V1 copies_4b=${n4b} -> $v1"
  say "V2 ngl_tokens=${ngl_total}${ngl_bad:+ (offenders:$ngl_bad)} -> $v2"
  say "V3 holders_gguf=${holders} -> $v3"
  say "V4 fit_on_4b=$(vkey 4B_FIT "$V") -> $v4"
  say "V5 devices_attributed=$(vkey DEVICES_ATTRIBUTED "$V") distinct_gpus=$(vkey DISTINCT_GPUS "$V") -> $v5"
  say "V6 fit_decided=$(vkey FIT_DECIDED "$V") ctx_ok=$(vkey CTX_OK "$V") ctx_4b=$(vkey CTX_4B "$V") fit_min_4b=$(vkey CTX_MIN_4B "$V") -> $v6"
  say "engines=${n_engines}"
  case "$v1$v2$v3$v4$v5$v6" in
    passpasspasspasspasspass)
      say "RESULT=PASS"
      exit 0 ;;
  esac
  case "$v5$v6" in
    nana) say "RESULT=N/A (V5/V6 sem motores atribuiveis)"; exit 2 ;;
  esac
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
DEV_TSV="$OUT/devices.tsv"
FIT_TSV="$OUT/fit-props.tsv"
: >"$ENGINES_TSV"
printf '#pid\tppid\tmodel\tngl\tfit\tctx\tsize\n' >>"$ENGINES_TSV"
printf '#pid\tcvd\tnvidia_visible\tdev_nodes\ton_gpu\tgpu_indexes\n' >"$DEV_TSV"
printf '#pid\tport\thttp\tn_ctx\tfit_min\tfixed_ctx\n' >"$FIT_TSV"

PIDS="$(pgrep -f 'llama-server|llama-cli' 2>/dev/null | sort -n || true)"
if [ -z "$PIDS" ]; then
  say "nenhum llama-server/llama-cli a correr (EngineAbsent nao e falha de argv)"
fi

for pid in $PIDS; do
  [ -r "/proc/$pid/cmdline" ] || continue
  mapfile -d '' TOK <"/proc/$pid/cmdline" 2>/dev/null || continue
  [ "${#TOK[@]}" -gt 0 ] || continue

  port=""; apikey=""
  model=""; fit=none; ctx=none
  i=0
  while [ "$i" -lt "${#TOK[@]}" ]; do
    case "${TOK[$i]}" in
      -m) model="${TOK[$((i + 1))]:-}" ;;
      --model) model="${TOK[$((i + 1))]:-}" ;;
      --fit) fit="${TOK[$((i + 1))]:-on}" ;;
      -c) ctx="${TOK[$((i + 1))]:-}" ;;
      --port) port="${TOK[$((i + 1))]:-}" ;;
      # S1 managed usa --api-key; fica so em memoria, nunca no bundle.
      --api-key) apikey="${TOK[$((i + 1))]:-}" ;;
    esac
    i=$((i + 1))
  done
  printf '%s\n' "${TOK[@]}" >"$OUT/engine-$pid.argv"
  # redigir o valor da chave de loopback antes de gravar no pendrive
  sed -i '/^--api-key$/{n;s/.*/<redacted>/}' "$OUT/engine-$pid.argv" 2>/dev/null || true

  # contar -ngl como TOKEN exacto, nunca substring (licao 30): "-ngl99" nao existe
  ngl_extra="$(grep -cx -- '-ngl' "$OUT/engine-$pid.argv" 2>/dev/null || printf 0)"
  case "$ngl_extra" in ''|*[!0-9]*) ngl_extra=0 ;; esac
  fixed_ctx="$(grep -cx -- '-c' "$OUT/engine-$pid.argv" 2>/dev/null || printf 0)"
  case "$fixed_ctx" in ''|*[!0-9]*) fixed_ctx=0 ;; esac
  fit_min="$(awk 'p{print;exit} $0=="--fit-ctx"{p=1}' "$OUT/engine-$pid.argv" 2>/dev/null)"
  case "$fit_min" in ''|*[!0-9]*) fit_min="-" ;; esac

  ppid="$(awk '{print $4}' "/proc/$pid/stat" 2>/dev/null || echo '?')"
  size=0
  [ -n "$model" ] && [ -f "$model" ] && size="$(stat -c %s "$model" 2>/dev/null || echo 0)"

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$pid" "$ppid" "${model:-?}" "$ngl_extra" "$fit" "${ctx:-auto}" "$size" >>"$ENGINES_TSV"
  say "pid=$pid model=${model:-?} ngl=$ngl_extra fit=$fit ctx=${ctx:-auto} size=$size"

  # --- V5: dispositivo real do motor (env declarado vs fd aberto) --------
  cvd="$(tr '\0' '\n' <"/proc/$pid/environ" 2>/dev/null \
    | sed -n 's/^CUDA_VISIBLE_DEVICES=//p' | head -1)"
  nvd="$(tr '\0' '\n' <"/proc/$pid/environ" 2>/dev/null \
    | sed -n 's/^NVIDIA_VISIBLE_DEVICES=//p' | head -1)"
  dev_nodes="$(ls -l "/proc/$pid/fd" 2>/dev/null \
    | grep -o '/dev/nvidia[a-z0-9-]*' | sort -u | tr '\n' ',' | sed 's/,$//')"
  gpu_idx="$(printf '%s' "$dev_nodes" | tr ',' '\n' \
    | sed -n 's#^/dev/nvidia\([0-9]\+\)$#\1#p' | sort -un | tr '\n' ',' | sed 's/,$//')"
  on_gpu=0; [ -n "$dev_nodes" ] && on_gpu=1
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$pid" "${cvd:-?}" "${nvd:-?}" "${dev_nodes:-none}" "$on_gpu" "${gpu_idx:-none}" >>"$DEV_TSV"

  # --- V6: contexto que o --fit escolheu (o motor fala por /props) ------
  code="-"; n_ctx="-"
  if [ -n "$port" ]; then
    body="$(http_props "$port" "$apikey")"
    code="$(printf '%s' "$body" | head -1 | awk '{print $2}')"
    [ -n "$code" ] || code="-"
    n_ctx="$(printf '%s' "$body" | tr -d '\r\n' \
      | grep -o '"n_ctx":[0-9]*' | head -1 | cut -d: -f2)"
    [ -n "$n_ctx" ] || n_ctx="-"
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$pid" "${port:-?}" "$code" "$n_ctx" "$fit_min" "$fixed_ctx" >>"$FIT_TSV"
  say "  port=${port:-?} props_http=$code n_ctx=$n_ctx fit_min=$fit_min fixed_ctx=$fixed_ctx cvd=${cvd:-?} dev=${dev_nodes:-none}"
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

# --- GPU / CUDA (contexto + atribuicao de PID a cartao) ------------------
hdr "GPU"
NVIDIA_SMI="${NOSD_NVIDIA_SMI:-nvidia-smi}"
if command -v "$NVIDIA_SMI" >/dev/null 2>&1; then
  "$NVIDIA_SMI" --query-gpu=index,name,driver_version,memory.total,memory.used \
    --format=csv,noheader >"$OUT/nvidia-smi.txt" 2>&1 || true
  "$NVIDIA_SMI" --query-compute-apps=pid,gpu_uuid,process_name,used_memory \
    --format=csv,noheader >"$OUT/nvidia-compute-apps.txt" 2>&1 || true
  cat "$OUT/nvidia-smi.txt" "$OUT/nvidia-compute-apps.txt"
else
  say "nvidia-smi ausente ($NVIDIA_SMI) — EngineAbsent/CPU e honestidade, nao falha"
  : >"$OUT/nvidia-smi.txt"
fi

hdr "dispositivos por motor"
column -t -s $'\t' "$DEV_TSV" 2>/dev/null || cat "$DEV_TSV"
hdr "contexto escolhido (--fit)"
column -t -s $'\t' "$FIT_TSV" 2>/dev/null || cat "$FIT_TSV"

# --- marcadores que so o codigo novo emite (licao 66) --------------------
hdr "marcadores no journal"
journalctl -b -u nosd -u nosd-engine-s2 -u nosd-engine-s1 -u jarbas-cage -u jarbas-hub \
  --no-pager >"$OUT/journal.txt" 2>&1 || true
grep -aE 'suggest_ngl=|spawned .*ngl=|hub slot=|transmute offload hint|n_ctx|fitting|offloaded|CUDA0' \
  "$OUT/journal.txt" >"$OUT/markers.txt" 2>/dev/null || true
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
ENGINES="$(awk 'NR>1 && $1 !~ /^#/' "$ENGINES_TSV" | wc -l | tr -d ' ')"
case "$ENGINES" in ''|*[!0-9]*) ENGINES=0 ;; esac

# V5: declarado == aberto. Sem motores -> na (EngineAbsent nao e PASS).
DEVICE_MATCH=na
DEV_ATTRIBUTED=0
DEV_MISMATCH=""
DISTINCT_GPUS=0
if [ "$ENGINES" -gt 0 ] && [ -f "$DEV_TSV" ]; then
  DISTINCT_GPUS="$(awk -F'\t' 'NR>1 && $6!="none"{n=split($6,a,",");for(i=1;i<=n;i++) if(a[i]!="") {print a[i]}}' \
    "$DEV_TSV" | sort -un | wc -l | tr -d ' ')"
  while IFS=$'\t' read -r pid cvd nvd nodes on_gpu idx; do
    case "${pid:-}" in ''|\#*) continue ;; esac
    declared=0
    [ "${cvd:-?}" != "?" ] && [ "${cvd:-}" != "none" ] && declared=1
    if [ "$declared" = 1 ] && [ "$on_gpu" = 1 ]; then
      DEV_ATTRIBUTED=$((DEV_ATTRIBUTED + 1))
    elif [ "$declared" = 1 ] && [ "$on_gpu" = 0 ]; then
      DEV_MISMATCH="$DEV_MISMATCH $pid(declara cvd=$cvd, sem /dev/nvidia)"
    elif [ "$declared" = 0 ] && [ "$on_gpu" = 1 ]; then
      DEV_ATTRIBUTED=$((DEV_ATTRIBUTED + 1))
    else
      DEV_ATTRIBUTED=$((DEV_ATTRIBUTED + 1))   # CPU por desenho, consistente
    fi
  done <"$DEV_TSV"
  if [ -z "$DEV_MISMATCH" ]; then DEVICE_MATCH=pass; else DEVICE_MATCH=fail; fi
fi

# V6: o fit escolheu (sem -c, --fit on) e o /props devolve um n_ctx real
# nao abaixo do minimo pedido ao --fit-ctx.
FIT_OK=na
FIT_DECIDED=0
CTX_OK=0
CTX_4B=none
CTX_MIN_4B=none
if [ "$ENGINES" -gt 0 ] && [ -f "$FIT_TSV" ]; then
  join_out="$(awk -F'\t' -v hint="$GGUF_4B_HINT" '
    FNR==NR { if (FNR>1 && $1 !~ /^#/) { fit[$1]=$5; model[$1]=$3 } next }
    FNR>1 && $1 !~ /^#/ {
      pid=$1
      f=(pid in fit ? fit[pid] : "none")
      decided=($6+0==0) ? 1 : 0
      ok=0
      if (decided && f=="on" && $4!="-" && $4+0>0) {
        if ($5=="-" || $4+0 >= $5+0) ok=1
      }
      d+=decided; c+=ok
      if (ctx4b=="" && index(model[pid], hint)>0) { ctx4b=$4; min4b=$5 }
    }
    END { printf "%d\t%d\t%s\t%s\n", d, c, (ctx4b==""?"none":ctx4b), (min4b==""?"none":min4b) }
  ' "$ENGINES_TSV" "$FIT_TSV")"
  FIT_DECIDED="$(printf '%s' "$join_out" | cut -f1)"
  CTX_OK="$(printf '%s' "$join_out" | cut -f2)"
  CTX_4B="$(printf '%s' "$join_out" | cut -f3)"
  CTX_MIN_4B="$(printf '%s' "$join_out" | cut -f4)"
  case "$FIT_DECIDED" in ''|*[!0-9]*) FIT_DECIDED=0 ;; esac
  case "$CTX_OK" in ''|*[!0-9]*) CTX_OK=0 ;; esac
  if [ "$FIT_DECIDED" -eq "$ENGINES" ] && [ "$CTX_OK" -eq "$ENGINES" ]; then
    FIT_OK=pass
  else
    FIT_OK=fail
  fi
fi

{
  printf '4B_PROCESSES=%s\n' "$COPIES_4B"
  printf 'NGL_TOKENS=%s\n' "$NGL_TOTAL"
  printf 'GGUF_HOLDERS=%s\n' "$HOLDERS"
  printf '4B_FIT=%s\n' "$FIT_4B"
  printf 'ENGINES=%s\n' "$ENGINES"
  printf 'DEVICES_ATTRIBUTED=%s/%s\n' "$DEV_ATTRIBUTED" "$ENGINES"
  printf 'DISTINCT_GPUS=%s\n' "$DISTINCT_GPUS"
  printf 'DEVICE_MATCH=%s\n' "$DEVICE_MATCH"
  printf 'FIT_DECIDED=%s/%s\n' "$FIT_DECIDED" "$ENGINES"
  printf 'CTX_OK=%s/%s\n' "$CTX_OK" "$ENGINES"
  printf 'CTX_4B=%s\n' "$CTX_4B"
  printf 'CTX_MIN_4B=%s\n' "$CTX_MIN_4B"
  printf 'FIT_OK=%s\n' "$FIT_OK"
} >"$OUT/verdict.txt"
cp "$ENGINES_TSV" "$OUT/engines.tsv" 2>/dev/null || true

hdr "veredicto"
cat "$OUT/verdict.txt"
[ -n "$DEV_MISMATCH" ] && say "device_mismatch:$DEV_MISMATCH"
say "bundle=$OUT"
say "=== DONE ==="
say "No host: bash scripts/metal-core7-harvest.sh --verify <bundle>"
