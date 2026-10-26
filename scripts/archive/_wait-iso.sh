#!/usr/bin/env bash
# Poll until ISO build finishes (COPIED_IMG or failure).
set -eu
LOG=/tmp/nixaios-iso-s529-nofat.out
ALT=/tmp/nixaios-iso-s529.out
for i in $(seq 1 120); do
  for f in "$LOG" "$ALT"; do
    if [ -f "$f" ] && grep -qE 'COPIED_IMG=|NO_ISO|error: builder' "$f" 2>/dev/null; then
      echo "DONE via $f"
      tail -n 30 "$f"
      exit 0
    fi
  done
  if ! pgrep -f 'build-iso-s529|build-iso-tmp|mksquashfs|nixos-minimal' >/dev/null 2>&1; then
    echo "NO_BUILD_PROC — last log:"
    tail -n 40 "$LOG" 2>/dev/null || tail -n 40 "$ALT" 2>/dev/null || true
    exit 1
  fi
  sleep 30
done
echo TIMEOUT
tail -n 40 "$LOG" 2>/dev/null || true
exit 1
