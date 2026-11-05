#!/usr/bin/env bash
# Colheita M3b metal (ADR-0128 filho): uma boot, um bundle, honesty ADR-0113.
# Uso (no guest/metal, como root ou nosd+journal): bash scripts/metal-m3b-harvest.sh [OUTDIR]
# Não promove OBSERVED→VERIFIED sozinho — só empacota provas A1–A6 + journal.
# F9: sumário A6=play_ok + snd_denied; journalctl --since. F10: trap limpa WAV.
set -euo pipefail

OUT="${1:-./m3b-harvest-$(date -u +%Y%m%dT%H%M%SZ)}"
XDG_NOSD="${XDG_RUNTIME_DIR_NOSD:-/run/nosd/user}"
NOSCTL="${NOSCTL:-nosctl}"
SINCE="$(date -Is)"
mkdir -p "$OUT"
MIC_WAV="$OUT/a4-mic.wav"

cleanup_mic() {
  rm -f "$MIC_WAV" 2>/dev/null || true
}
trap cleanup_mic EXIT

exec > >(tee "$OUT/harvest.log") 2>&1

echo "=== M3b harvest $(date -Is) OUT=$OUT XDG=$XDG_NOSD SINCE=$SINCE ==="

A1=absent
A2=absent
A4=absent
A6_PLAY=absent

echo "--- units ---"
systemctl --no-pager --state=failed list-units 'nosd*' 'jarbas*' 'nixaios*' || true
systemctl show jarbas-cage -p DeviceAllow -p SupplementaryGroups -p ConditionResult || true

echo "--- A1 aplay -l ---"
if aplay -l >"$OUT/a1-aplay-l.txt" 2>"$OUT/a1-aplay-l.err"; then
  if grep -q 'card [0-9]' "$OUT/a1-aplay-l.txt"; then
    A1=card_present
    echo "A1=card_present"
  else
    A1=audio_absent
    echo "A1=audio_absent"
  fi
else
  A1=audio_absent
  echo "A1=audio_absent (aplay failed)"
fi

run_as_nosd() {
  if [ "$(id -u)" -eq 0 ] && id nosd >/dev/null 2>&1; then
    runuser -u nosd -- env XDG_RUNTIME_DIR="$XDG_NOSD" "$@"
  else
    env XDG_RUNTIME_DIR="$XDG_NOSD" "$@"
  fi
}

echo "--- A2 PipeWire / wpctl ---"
# Honestidade do instrumento: `wpctl`/`pw-cli` ausentes do PATH do probe NÃO
# provam que o grafo PipeWire não existe (metal 2026-11-03: A2=pipewire_absent
# com /run/nosd/user/pipewire-0 presente). A prova primária é o socket; as
# ferramentas são reportadas à parte.
A2_TOOLS=absent
if [ -S "$XDG_NOSD/pipewire-0" ]; then
  A2=pipewire_socket
  echo "A2=pipewire_socket ($XDG_NOSD/pipewire-0)"
else
  A2=pipewire_absent
  echo "A2=pipewire_absent (sem socket em $XDG_NOSD)"
fi
if run_as_nosd wpctl status >"$OUT/a2-wpctl.txt" 2>"$OUT/a2-wpctl.err"; then
  A2_TOOLS=wpctl_ok
elif run_as_nosd sh -c 'command -v wpctl >/dev/null 2>&1 || [ -x /run/current-system/sw/bin/wpctl ]' 2>/dev/null; then
  A2_TOOLS=wpctl_present_failed
else
  A2_TOOLS=wpctl_absent_path
fi
echo "A2_TOOLS=${A2_TOOLS}"
ls -la "$XDG_NOSD" >"$OUT/a2-runtime-dir.txt" 2>&1 || true
run_as_nosd pw-cli info 0 >"$OUT/a2-pw-cli.txt" 2>"$OUT/a2-pw-cli.err" || true

echo "--- A4 arecord 2s ---"
# 16 kHz mono: é o formato que `nos-voice` grava e o que o whisper.cpp aceita.
# Sem `-r 16000 -c 1` o ALSA usa 8000 Hz e o STT falha por taxa errada —
# falso negativo do instrumento, não do microfone (NEXUS-0041 F8).
if run_as_nosd arecord -q -f S16_LE -r 16000 -c 1 -d 2 "$MIC_WAV" >"$OUT/a4-arecord.txt" 2>"$OUT/a4-arecord.err"; then
  BYTES=$(wc -c <"$MIC_WAV" | tr -d ' ')
  if [ "${BYTES:-0}" -gt 44 ]; then
    A4=mic_ok
    echo "A4=mic_ok bytes=$BYTES"
  else
    A4=mic_absent
    echo "A4=mic_absent bytes=$BYTES"
  fi
else
  A4=mic_absent
  echo "A4=mic_absent (arecord failed)"
fi

echo "--- A5/A6 nosctl voice ---"
# F7: voice-turn exige --prompt; A6 TTS = voice-tts (não positional em voice-turn).
if command -v "$NOSCTL" >/dev/null 2>&1; then
  "$NOSCTL" voice-status >"$OUT/a5-voice-status.json" 2>"$OUT/a5-voice-status.err" || true
  "$NOSCTL" voice-tts "teste de audio" >"$OUT/a6-voice-tts.json" 2>"$OUT/a6-voice-tts.err" || true
  if [ -f "$MIC_WAV" ]; then
    "$NOSCTL" voice-stt --wav-path "$MIC_WAV" \
      >"$OUT/a5-voice-stt.json" 2>"$OUT/a5-voice-stt.err" || true
  fi
  "$NOSCTL" voice-turn --prompt "teste de audio" \
    >"$OUT/a6-voice-turn.json" 2>"$OUT/a6-voice-turn.err" || true
else
  echo "nosctl absent — A5/A6 skipped" | tee "$OUT/a5-skipped.txt"
fi

# F9: play_ok via json.loads (lição 25 — nunca substring/"key":")
if [ -f "$OUT/a6-voice-tts.json" ]; then
  A6_PLAY="$(python3 -c "
import json, sys
p = sys.argv[1]
try:
    raw = open(p, encoding='utf-8', errors='replace').read()
    j = json.loads(raw)
except Exception:
    print('absent')
    raise SystemExit(0)
r = j.get('result') if isinstance(j, dict) else None
if not isinstance(r, dict):
    r = j if isinstance(j, dict) else {}
v = r.get('play_ok')
if v is True:
    print('true')
elif v is False:
    print('false')
else:
    print('absent')
" "$OUT/a6-voice-tts.json" 2>/dev/null || echo absent)"
fi

echo "--- journal ---"
journalctl -b --since "$SINCE" -u nosd -u jarbas-cage -u jarbas-hub -u nosd-hotswap-aios \
  -u nosd-engine-s2 --no-pager >"$OUT/journal.txt" 2>&1 || true
journalctl -b --since "$SINCE" -u nixaios-log-mount -u nixaios-log-export --no-pager \
  >"$OUT/journal-log.txt" 2>&1 || true

SND_DENIED=0
if [ -f "$OUT/journal.txt" ]; then
  # grep -c imprime 0 e exit 1 sem matches; capturar stdout, ignorar rc.
  SND_DENIED="$(grep -cE 'Permission denied.*(/dev/)?snd' "$OUT/journal.txt" 2>/dev/null || printf '%s' 0)"
  case "$SND_DENIED" in ''|*[!0-9]*) SND_DENIED=0 ;; esac
fi

echo "--- mesh seeds (health) ---"
if command -v "$NOSCTL" >/dev/null 2>&1; then
  "$NOSCTL" health >"$OUT/health.json" 2>"$OUT/health.err" || true
fi

echo "A1=${A1} A2=${A2} A4=${A4} A6=play_ok=${A6_PLAY} snd_denied=${SND_DENIED}"
echo "=== DONE bundle=$OUT ==="
ls -la "$OUT"

# Sucesso: manter o WAV no bundle (prova A4); desarmar cleanup no EXIT limpo.
trap - EXIT
