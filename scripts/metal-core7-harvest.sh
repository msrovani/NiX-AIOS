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
# Voz (piper/whisper SAO POR PEDIDO - nao se lê de um `ps` parado; mede-se o
# spawn real durante um pedido RPC, e o argv sai de /proc/<pid>/cmdline):
#   W1  uma so copia    -> 1 binario piper resolvido, 1 .onnx do idioma,
#                          e nunca 2 piper/whisper em paralelo
#   W2  argv            -> piper: --model + --output_file; whisper: -m -l -f
#                          --no-timestamps (contrato de nos-voice/src/lib.rs)
#   W3  TTS corre       -> WAV > 44 B devolvido por voice.tts
#   W4  STT corre       -> voice-stt rc=0 num WAV 16 kHz (na sem mic)
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
    if [ "$(vkey VOICE "$V")" = fail ]; then
      say "V1..V6 na (EngineAbsent) mas voz falhou: W1=$(vkey W1 "$V") W2=$(vkey W2 "$V") W3=$(vkey W3 "$V") W4=$(vkey W4 "$V")"
      say "RESULT=FAIL (voz)"
      exit 1
    fi
    say "V1..V6 na (EngineAbsent: nenhum motor no argv; voz=$(vkey VOICE "$V")) -> RESULT=N/A"
    exit 2
  fi

  v1=fail; [ "$n4b" -eq 1 ] && v1=pass
  v2=fail; [ "$ngl_total" -eq 0 ] && v2=pass
  v3=fail; [ "$holders" -eq 1 ] && v3=pass
  v4=fail; [ "$(vkey 4B_FIT "$V")" = "on" ] && v4=pass
  v5="$(vkey DEVICE_MATCH "$V")"; [ -n "$v5" ] || v5=fail
  v6="$(vkey FIT_OK "$V")"; [ -n "$v6" ] || v6=fail
  w1="$(vkey W1 "$V")"; [ -n "$w1" ] || w1=na
  w2="$(vkey W2 "$V")"; [ -n "$w2" ] || w2=na
  w3="$(vkey W3 "$V")"; [ -n "$w3" ] || w3=na
  w4="$(vkey W4 "$V")"; [ -n "$w4" ] || w4=na

  say "V1 copies_4b=${n4b} -> $v1"
  say "V2 ngl_tokens=${ngl_total}${ngl_bad:+ (offenders:$ngl_bad)} -> $v2"
  say "V3 holders_gguf=${holders} -> $v3"
  say "V4 fit_on_4b=$(vkey 4B_FIT "$V") -> $v4"
  say "V5 devices_attributed=$(vkey DEVICES_ATTRIBUTED "$V") distinct_gpus=$(vkey DISTINCT_GPUS "$V") -> $v5"
  say "V6 fit_decided=$(vkey FIT_DECIDED "$V") ctx_ok=$(vkey CTX_OK "$V") ctx_4b=$(vkey CTX_4B "$V") fit_min_4b=$(vkey CTX_MIN_4B "$V") -> $v6"
  say "engines=${n_engines}"
  say "W1 uma_copia=${w1} W2 argv=${w2} W3 tts_bytes=$(vkey TTS_BYTES "$V") -> ${w3} W4 stt_rc=$(vkey STT_RC "$V") -> ${w4}"
  say "voice=$(vkey VOICE "$V")"
  voice="$(vkey VOICE "$V")"
  # Falha medida na voz nunca fica escondida por um N/A dos motores LLM.
  if [ "$voice" = fail ]; then
    say "RESULT=FAIL (voz: W1=$(vkey W1 "$V") W2=$(vkey W2 "$V") W3=$(vkey W3 "$V") W4=$(vkey W4 "$V"))"
    exit 1
  fi
  case "$v1$v2$v3$v4$v5$v6" in
    passpasspasspasspasspass)
      say "RESULT=PASS"
      exit 0 ;;
  esac
  if [ "$v5$v6" = nana ]; then
    say "RESULT=N/A (V5/V6 sem motores atribuiveis; voz=$voice)"
    exit 2
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

# --- voz: piper/whisper (W1..W4) ----------------------------------------
# piper/whisper NAO sao servicos: nascem e morrem dentro de voice.tts/voice.stt.
# Por isso "uma so copia" mede-se (a) na resolucao de nos-voice e (b) no spawn
# real observado durante um pedido RPC, com o argv de /proc/<pid>/cmdline.
hdr "voz (piper/whisper)"
MODELS_DIR="${NOSD_MODELS_DIR:-/var/lib/nosd/models}"
VOICE_TSV="$OUT/voice-spawn.tsv"
printf '#engine\tpid\tppid\targv\n' >"$VOICE_TSV"

NOSD_PID="$(pgrep -x nosd 2>/dev/null | head -1)"
envof() { [ -n "$NOSD_PID" ] && tr '\0' '\n' <"/proc/$NOSD_PID/environ" 2>/dev/null | sed -n "s/^$1=//p" | head -1; }
NOSD_PATH="$(envof PATH)"; [ -n "$NOSD_PATH" ] || NOSD_PATH="$PATH"

# 1) resolucao replicada de nos-voice (find_piper_cli / find_whisper_cli)
PIPER_ENV="$(envof NOSD_PIPER_CLI)"; { [ -n "$PIPER_ENV" ] && [ -f "$PIPER_ENV" ]; } || PIPER_ENV=""
PIPER_FIXED="$MODELS_DIR/piper/piper"; [ -f "$PIPER_FIXED" ] || PIPER_FIXED=""
PIPER_PATHBIN="$(PATH="$NOSD_PATH" command -v piper 2>/dev/null || true)"
PIPER_CHOSEN="${PIPER_ENV:-${PIPER_FIXED:-$PIPER_PATHBIN}}"
PIPER_CANDIDATES="$(printf '%s\n%s\n%s\n' "$PIPER_ENV" "$PIPER_FIXED" "$PIPER_PATHBIN" | sed '/^$/d' | sort -u | wc -l | tr -d ' ')"

WHISPER_ENV="$(envof NOSD_WHISPER_CLI)"; { [ -n "$WHISPER_ENV" ] && [ -f "$WHISPER_ENV" ]; } || WHISPER_ENV=""
WHISPER_PATHBIN=""
for n in whisper-cli whisper-cpp whisper; do
  b="$(PATH="$NOSD_PATH" command -v "$n" 2>/dev/null || true)"
  [ -n "$b" ] && { WHISPER_PATHBIN="$b"; break; }
done
WHISPER_CHOSEN="${WHISPER_ENV:-$WHISPER_PATHBIN}"
WHISPER_CANDIDATES="$(printf '%s\n%s\n' "$WHISPER_ENV" "$WHISPER_PATHBIN" | sed '/^$/d' | sort -u | wc -l | tr -d ' ')"

# idioma primario como nos-voice o deriva do locale (replica; o spawn manda)
RAW_LOCALE="${NOSD_LOCALE:-}"
if [ -z "$RAW_LOCALE" ]; then
  for f in /var/lib/nosd/locale.conf /home/nixos/.config/gnome-initial-setup-locale /etc/locale.conf /etc/default/locale; do
    [ -f "$f" ] || continue
    v="$(sed -n 's/^LANG=//p' "$f" 2>/dev/null | head -1 | tr -d '"')"
    [ -n "$v" ] || v="$(grep -m1 -E '^[a-zA-Z]{2}(_[A-Za-z]+)?\.' "$f" 2>/dev/null | tr -d '"')"
    [ -n "$v" ] && { RAW_LOCALE="$v"; break; }
  done
fi
[ -n "$RAW_LOCALE" ] || RAW_LOCALE="pt_BR.UTF-8"
LANG_LC="${RAW_LOCALE,,}"
case "$LANG_LC" in
  pt*) LANG_PRIMARY=pt ;;
  es*|fr*|de*) LANG_PRIMARY="${LANG_LC:0:2}" ;;
  *) LANG_PRIMARY=en ;;
esac
case "$LANG_PRIMARY:$LANG_LC" in
  pt:pt_pt*) PIPER_VOICE=pt_PT-tugao-medium ;;
  pt:*) PIPER_VOICE=pt_BR-faber-medium ;;
  es:*) PIPER_VOICE=es_ES-davefx-medium ;;
  fr:*) PIPER_VOICE=fr_FR-siwis-medium ;;
  de:*) PIPER_VOICE=de_DE-thorsten-medium ;;
  *) PIPER_VOICE=en_US-lessac-medium ;;
esac
PIPER_ONNX_PREF="$MODELS_DIR/$PIPER_VOICE.onnx"; [ -f "$PIPER_ONNX_PREF" ] || PIPER_ONNX_PREF=""
ONNX_LIST="$(ls "$MODELS_DIR"/*.onnx 2>/dev/null || true)"
PIPER_ONNX_MATCHING=0; PIPER_ONNX_CHOSEN=""
while IFS= read -r f; do
  [ -n "$f" ] || continue
  st="$(basename "$f" .onnx)"; st_l="${st,,}"
  case "$st_l" in "$LANG_PRIMARY"*) PIPER_ONNX_MATCHING=$((PIPER_ONNX_MATCHING + 1))
    [ -n "$PIPER_ONNX_CHOSEN" ] || PIPER_ONNX_CHOSEN="$f" ;; esac
done <<<"$(printf '%s\n' "$ONNX_LIST" | sort)"
[ -n "$PIPER_ONNX_PREF" ] && PIPER_ONNX_CHOSEN="$PIPER_ONNX_PREF"
PIPER_SIDECAR=missing; { [ -n "$PIPER_ONNX_CHOSEN" ] && [ -f "$PIPER_ONNX_CHOSEN.json" ]; } && PIPER_SIDECAR=present

WHISPER_MODEL=""; WHISPER_MODELS_PRESENT=0
for m in ggml-base.bin ggml-small.bin ggml-tiny.bin; do
  [ -f "$MODELS_DIR/$m" ] || continue
  WHISPER_MODELS_PRESENT=$((WHISPER_MODELS_PRESENT + 1))
  [ -n "$WHISPER_MODEL" ] || WHISPER_MODEL="$m"
done

say "piper_chosen=${PIPER_CHOSEN:-none} candidates=$PIPER_CANDIDATES"
say "piper_onnx_matching=$PIPER_ONNX_MATCHING chosen=$(basename "${PIPER_ONNX_CHOSEN:-none}") sidecar=$PIPER_SIDECAR"
say "whisper_chosen=${WHISPER_CHOSEN:-none} candidates=$WHISPER_CANDIDATES model=${WHISPER_MODEL:-none} models_present=$WHISPER_MODELS_PRESENT"

# 2) spawn real durante um pedido RPC (piper/whisper vivem <2s)
MAX_PAR_PIPER=0
MAX_PAR_WHISPER=0
SPAWNS_PIPER=0
SPAWNS_WHISPER=0

# Classificar por basename do argv, nunca por `comm`: o `piper` da ISO e um
# wrapper bash que faz `exec` em `piper.real` (mesmo PID, argv novo) — `comm`
# seria `bash` e o binario real chama-se `piper.real` (lesson 61).
engine_scan() {
  ps -eo pid=,args= 2>/dev/null | awk '
    {
      pid=$1; eng=""
      for (i=2; i<=NF; i++) {
        n=split($i, a, "/"); b=a[n]
        if (b=="piper" || b=="piper.real") eng="piper"
        else if (b=="whisper-cli" || b=="whisper-cli.real" || b=="whisper") eng="whisper"
      }
      if (eng!="") print pid "\t" eng
    }'
}

ppid_of() { sed 's/^[^)]*) //' "/proc/$1/stat" 2>/dev/null | awk '{print $2}'; }

# Só motores descendentes do nosd (e a RPC que os gera): um piper alheio
# (jarbas a falar durante a colheita) não pode contar como segunda cópia.
desc_of_nosd() {
  local p="$1" i=0
  [ -n "$NOSD_PID" ] || return 0
  while [ "${p:-0}" -gt 1 ] && [ "$i" -lt 10 ]; do
    p="$(ppid_of "$p")"
    [ -n "$p" ] || return 1
    [ "$p" = "$NOSD_PID" ] && return 0
    i=$((i + 1))
  done
  return 1
}

# Uma passagem de amostragem: conta concorrência e regista argv novo.
# Usa VOICE_SEEN_FILE global; actualiza MAX_PAR_PIPER/MAX_PAR_WHISPER.
scan_voice_once() {
  local pid eng argv cur_p=0 cur_w=0
  while IFS=$'\t' read -r pid eng; do
    [ -n "$pid" ] || continue
    desc_of_nosd "$pid" || continue
    case "$eng" in piper) cur_p=$((cur_p + 1)) ;; whisper) cur_w=$((cur_w + 1)) ;; esac
    grep -qx "$pid" "$VOICE_SEEN_FILE" 2>/dev/null && continue
    argv="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null)" || continue
    [ -n "$argv" ] || continue
    printf '%s\t%s\t%s\t%s\n' "$eng" "$pid" "$(ppid_of "$pid")" "$argv" >>"$VOICE_TSV"
    echo "$pid" >>"$VOICE_SEEN_FILE"
  done <<<"$(engine_scan)"
  [ "$cur_p" -gt "$MAX_PAR_PIPER" ] && MAX_PAR_PIPER=$cur_p
  [ "$cur_w" -gt "$MAX_PAR_WHISPER" ] && MAX_PAR_WHISPER=$cur_w
}

capture_spawn() { # $1=label $2=segundos ; resto = comando
  local label="$1" secs="$2"; shift 2
  local end rpc
  VOICE_SEEN_FILE="$OUT/.seen-$label"
  : >"$VOICE_SEEN_FILE"
  ( "$@" >"$OUT/$label.out" 2>"$OUT/$label.err"; echo $? >"$OUT/$label.rc" ) &
  rpc=$!
  end=$(( $(date +%s) + secs ))
  while kill -0 "$rpc" 2>/dev/null && [ "$(date +%s)" -lt "$end" ]; do
    scan_voice_once
    sleep 0.01
  done
  wait "$rpc" 2>/dev/null || true
  scan_voice_once   # ultima passagem: apanha quem sobreviveu ao fim do RPC
}

NOSCTL_BIN="${NOSCTL_BIN:-$(command -v nosctl 2>/dev/null || true)}"
[ -x "$NOSCTL_BIN" ] || NOSCTL_BIN=/run/current-system/sw/bin/nosctl
[ -x "$NOSCTL_BIN" ] || NOSCTL_BIN=""

# 3) TTS: voice.tts --no-play (o play e outro criterio, M3b A6)
TTS_BYTES=absent
if [ -n "$NOSCTL_BIN" ]; then
  capture_spawn tts 30 "$NOSCTL_BIN" voice-tts --no-play "harvest voz metal"
  TTS_RC="$(cat "$OUT/tts.rc" 2>/dev/null || echo '?')"
  TTS_BYTES="$(sed -n 's/.*"bytes":\([0-9][0-9]*\).*/\1/p' "$OUT/tts.out" 2>/dev/null | head -1)"
  [ -n "$TTS_BYTES" ] || TTS_BYTES=absent
  say "tts rc=$TTS_RC bytes=$TTS_BYTES max_parallel_piper=$MAX_PAR_PIPER"
else
  say "nosctl ausente: W2..W4 na (resolucao estatica em cima)"
  TTS_RC=absent
fi

# 4) STT: WAV 16 kHz (arecord como nosd, mesmo formato de nos-voice)
MIC_WAV="$OUT/voice-mic.wav"
STT_RC=absent
mic_ok=0
if command -v arecord >/dev/null 2>&1; then
  if [ "$(id -u)" -eq 0 ] && id nosd >/dev/null 2>&1; then
    runuser -u nosd -- env XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR_NOSD:-/run/nosd/user}" \
      arecord -q -f S16_LE -r 16000 -c 1 -d 2 "$MIC_WAV" >/dev/null 2>"$OUT/voice-mic.err" || true
  else
    arecord -q -f S16_LE -r 16000 -c 1 -d 2 "$MIC_WAV" >/dev/null 2>"$OUT/voice-mic.err" || true
  fi
  [ -s "$MIC_WAV" ] && [ "$(stat -c %s "$MIC_WAV" 2>/dev/null || echo 0)" -gt 44 ] && mic_ok=1
fi
if [ -n "$NOSCTL_BIN" ] && [ "$mic_ok" -eq 1 ]; then
  capture_spawn stt 90 "$NOSCTL_BIN" voice-stt --wav-path "$MIC_WAV"
  STT_RC="$(cat "$OUT/stt.rc" 2>/dev/null || echo '?')"
  say "stt rc=$STT_RC mic_bytes=$(stat -c %s "$MIC_WAV" 2>/dev/null || echo 0) max_parallel_whisper=$MAX_PAR_WHISPER"
else
  say "stt na (sem mic 16 kHz ou sem nosctl) mic_ok=$mic_ok"
fi

SPAWNS_PIPER="$(awk -F'\t' '$1=="piper"' "$VOICE_TSV" | wc -l | tr -d ' ')"
SPAWNS_WHISPER="$(awk -F'\t' '$1=="whisper"' "$VOICE_TSV" | wc -l | tr -d ' ')"

piper_argv_ok=na; whisper_argv_ok=na
if [ "$SPAWNS_PIPER" -gt 0 ]; then
  piper_argv_ok=pass
  awk -F'\t' '$1=="piper"{print " "$4" "}' "$VOICE_TSV" >"$OUT/.piper.argv"
  grep -q -- '--model' "$OUT/.piper.argv" || piper_argv_ok=fail
  grep -q -- '--output_file' "$OUT/.piper.argv" || piper_argv_ok=fail
fi
if [ "$SPAWNS_WHISPER" -gt 0 ]; then
  whisper_argv_ok=pass
  awk -F'\t' '$1=="whisper"{print " "$4" "}' "$VOICE_TSV" >"$OUT/.whisper.argv"
  for flag in ' -m ' ' -l ' ' -f ' '--no-timestamps'; do
    grep -q -- "$flag" "$OUT/.whisper.argv" || whisper_argv_ok=fail
  done
fi
say "spawns piper=$SPAWNS_PIPER whisper=$SPAWNS_WHISPER argv_piper=$piper_argv_ok argv_whisper=$whisper_argv_ok"

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

# W1: uma so copia (resolucao unica + modelos unambiguous + nunca 2 em paralelo)
W1=na
if [ -n "$PIPER_CHOSEN" ] || [ -n "$WHISPER_CHOSEN" ]; then
  W1=pass
  if [ -n "$PIPER_CHOSEN" ]; then
    [ "$PIPER_CANDIDATES" -eq 1 ] || W1=fail
    [ "$PIPER_ONNX_MATCHING" -le 1 ] || W1=fail
    if [ -n "$PIPER_ONNX_CHOSEN" ] && [ "$PIPER_SIDECAR" != present ]; then W1=fail; fi
  fi
  if [ -n "$WHISPER_CHOSEN" ]; then
    [ "$WHISPER_CANDIDATES" -eq 1 ] || W1=fail
    [ -n "$WHISPER_MODEL" ] || W1=fail
  fi
  [ "$MAX_PAR_PIPER" -le 1 ] || W1=fail
  [ "$MAX_PAR_WHISPER" -le 1 ] || W1=fail
fi

# W2: argv real dos spawns (contrato de nos-voice/src/lib.rs)
W2=na
if [ "$piper_argv_ok" != na ] || [ "$whisper_argv_ok" != na ]; then
  W2=pass
  [ "$piper_argv_ok" = fail ] && W2=fail
  [ "$whisper_argv_ok" = fail ] && W2=fail
fi

# W3: TTS produziu WAV (> 44 B); 0 B/44 B = o ficheiro existe mas ninguem escreveu
W3=na
case "$TTS_BYTES" in
  ''|absent|*[!0-9]*) : ;;
  *) if [ "$TTS_BYTES" -gt 44 ]; then W3=pass; else W3=fail; fi ;;
esac

# W4: STT rc=0 num WAV 16 kHz (texto vazio e legal aqui: silencio nao e falha de argv)
W4=na
case "$STT_RC" in
  ''|absent|*[!0-9]*) : ;;
  *) if [ "$STT_RC" -eq 0 ]; then W4=pass; else W4=fail; fi ;;
esac

VOICE=na
case "$W1$W2$W3$W4" in
  *fail*) VOICE=fail ;;
  nana*) VOICE=na ;;
  *) VOICE=pass ;;
esac

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
  printf 'PIPER_CHOSEN=%s\n' "${PIPER_CHOSEN:-none}"
  printf 'PIPER_CANDIDATES=%s\n' "$PIPER_CANDIDATES"
  printf 'PIPER_ONNX_MATCHING=%s\n' "$PIPER_ONNX_MATCHING"
  printf 'PIPER_ONNX_CHOSEN=%s\n' "$(basename "${PIPER_ONNX_CHOSEN:-none}")"
  printf 'PIPER_SIDECAR=%s\n' "$PIPER_SIDECAR"
  printf 'WHISPER_CHOSEN=%s\n' "${WHISPER_CHOSEN:-none}"
  printf 'WHISPER_CANDIDATES=%s\n' "$WHISPER_CANDIDATES"
  printf 'WHISPER_MODEL=%s\n' "${WHISPER_MODEL:-none}"
  printf 'SPAWNS_PIPER=%s\n' "$SPAWNS_PIPER"
  printf 'SPAWNS_WHISPER=%s\n' "$SPAWNS_WHISPER"
  printf 'MAX_PARALLEL_PIPER=%s\n' "$MAX_PAR_PIPER"
  printf 'MAX_PARALLEL_WHISPER=%s\n' "$MAX_PAR_WHISPER"
  printf 'PIPER_ARGV=%s\n' "$piper_argv_ok"
  printf 'WHISPER_ARGV=%s\n' "$whisper_argv_ok"
  printf 'TTS_BYTES=%s\n' "$TTS_BYTES"
  printf 'TTS_RC=%s\n' "${TTS_RC:-absent}"
  printf 'STT_RC=%s\n' "${STT_RC:-absent}"
  printf 'W1=%s\n' "$W1"
  printf 'W2=%s\n' "$W2"
  printf 'W3=%s\n' "$W3"
  printf 'W4=%s\n' "$W4"
  printf 'VOICE=%s\n' "$VOICE"
} >"$OUT/verdict.txt"
cp "$ENGINES_TSV" "$OUT/engines.tsv" 2>/dev/null || true

hdr "veredicto"
cat "$OUT/verdict.txt"
[ -n "$DEV_MISMATCH" ] && say "device_mismatch:$DEV_MISMATCH"
say "bundle=$OUT"
say "=== DONE ==="
say "No host: bash scripts/metal-core7-harvest.sh --verify <bundle>"
