#!/usr/bin/env bash
# Colheita M3b metal (ADR-0128 filho): uma boot, um bundle, honesty ADR-0113.
# Uso (no guest/metal, como root ou nosd+journal): bash scripts/metal-m3b-harvest.sh [OUTDIR]
# Não promove OBSERVED→VERIFIED sozinho — só empacota provas A1–A6 + journal.
set -euo pipefail

OUT="${1:-./m3b-harvest-$(date -u +%Y%m%dT%H%M%SZ)}"
XDG_NOSD="${XDG_RUNTIME_DIR_NOSD:-/run/nosd/user}"
NOSCTL="${NOSCTL:-nosctl}"
mkdir -p "$OUT"
exec > >(tee "$OUT/harvest.log") 2>&1

echo "=== M3b harvest $(date -Is) OUT=$OUT XDG=$XDG_NOSD ==="

echo "--- units ---"
systemctl --no-pager --state=failed list-units 'nosd*' 'jarbas*' 'nixaios*' || true
systemctl show jarbas-cage -p DeviceAllow -p SupplementaryGroups -p ConditionResult || true

echo "--- A1 aplay -l ---"
if aplay -l >"$OUT/a1-aplay-l.txt" 2>"$OUT/a1-aplay-l.err"; then
  if grep -q 'card [0-9]' "$OUT/a1-aplay-l.txt"; then
    echo "A1=card_present"
  else
    echo "A1=audio_absent"
  fi
else
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
if run_as_nosd wpctl status >"$OUT/a2-wpctl.txt" 2>"$OUT/a2-wpctl.err"; then
  echo "A2=wpctl_ok"
else
  echo "A2=pipewire_absent"
fi
ls -la "$XDG_NOSD" >"$OUT/a2-runtime-dir.txt" 2>&1 || true
run_as_nosd pw-cli info 0 >"$OUT/a2-pw-cli.txt" 2>"$OUT/a2-pw-cli.err" || true

echo "--- A4 arecord 2s ---"
MIC_WAV="$OUT/a4-mic.wav"
if run_as_nosd arecord -d 2 -f S16_LE "$MIC_WAV" >"$OUT/a4-arecord.txt" 2>"$OUT/a4-arecord.err"; then
  BYTES=$(wc -c <"$MIC_WAV" | tr -d ' ')
  if [ "${BYTES:-0}" -gt 44 ]; then
    echo "A4=mic_ok bytes=$BYTES"
  else
    echo "A4=mic_absent bytes=$BYTES"
  fi
else
  echo "A4=mic_absent (arecord failed)"
fi

echo "--- A5/A6 nosctl voice ---"
# F7: voice-turn exige --prompt; A6 TTS = voice-tts (não positional em voice-turn).
if command -v "$NOSCTL" >/dev/null 2>&1; then
  "$NOSCTL" voice-status >"$OUT/a5-voice-status.json" 2>"$OUT/a5-voice-status.err" || true
  "$NOSCTL" voice-tts "teste de audio" >"$OUT/a6-voice-tts.json" 2>"$OUT/a6-voice-tts.err" || true
  if [ -f "$OUT/a4-mic.wav" ]; then
    "$NOSCTL" voice-stt --wav-path "$OUT/a4-mic.wav" \
      >"$OUT/a5-voice-stt.json" 2>"$OUT/a5-voice-stt.err" || true
  fi
  "$NOSCTL" voice-turn --prompt "teste de audio" \
    >"$OUT/a6-voice-turn.json" 2>"$OUT/a6-voice-turn.err" || true
else
  echo "nosctl absent — A5/A6 skipped" | tee "$OUT/a5-skipped.txt"
fi

echo "--- journal ---"
journalctl -b -u nosd -u jarbas-cage -u jarbas-hub -u nosd-hotswap-aios \
  -u nosd-engine-s2 --no-pager >"$OUT/journal.txt" 2>&1 || true
journalctl -b -u nixaios-log-mount -u nixaios-log-export --no-pager \
  >"$OUT/journal-log.txt" 2>&1 || true

echo "--- mesh seeds (health) ---"
if command -v "$NOSCTL" >/dev/null 2>&1; then
  "$NOSCTL" health >"$OUT/health.json" 2>"$OUT/health.err" || true
fi

echo "=== DONE bundle=$OUT ==="
ls -la "$OUT"
