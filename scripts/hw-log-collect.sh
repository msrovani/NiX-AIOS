#!/usr/bin/env bash
# Colector de diagnóstico de metal (V9/hw) — corre no tty1 do guest.
#
#   bash hw-log-collect.sh [DIR]
#
# Sem argumento escreve em /mnt/nixaios-log (NIXAIOSLOG). Se a FAT não estiver
# montada, usa /tmp/nixaios-diag e diz onde (não há persistência sem FAT).
# Tudo é best-effort: binário ausente não aborta.
set -uo pipefail

OUT="${1:-/mnt/nixaios-log}"
if [[ ! -d "$OUT" ]]; then
  echo "NIXAIOSLOG ausente em $OUT — a usar /tmp/nixaios-diag (perde-se no poweroff)" >&2
  OUT=/tmp/nixaios-diag
fi
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
D="$OUT/diag-$STAMP"
mkdir -p "$D" || { echo "sem escrita em $OUT" >&2; exit 1; }

say() { printf '%s\n' "### $*" >>"$D/$1"; shift; { "$@" 2>&1 || true; } >>"$D/$1"; }
guard() { # guard <log> <cmd...> — só corre se o binário existir
  local log="$1" bin="$2"
  shift 2
  if command -v "$bin" >/dev/null 2>&1; then
    say "$log" "$bin" "$@"
  else
    printf '%s\n' "### $bin: not found" >>"$D/$log"
  fi
}

echo "==> diag em $D"
{
  echo "stamp=$STAMP"
  uname -a
  cat /proc/cmdline
  echo "--- uptime"; uptime
  echo "--- boot id"; cat /proc/sys/kernel/random/boot_id
} >"$D/summary.log" 2>&1

journalctl -b --no-pager >"$D/journal-boot.log" 2>&1 || true
dmesg >"$D/dmesg.log" 2>&1 || true
systemctl --no-pager --failed >"$D/failed-units.log" 2>&1 || true
systemctl --no-pager list-units --type=service --all \
  'nosd*' 'jarbas*' 'nixaios*' 'pipewire*' 'wireplumber*' 'seatd*' >"$D/units.log" 2>&1 || true

guard hw.log lspci -nn
guard hw.log lsusb
guard hw.log lsblk -f
guard hw.log free -h
guard hw.log nproc

# Áudio: PipeWire/WirePlumber + ALSA (mic/som).
guard audio.log wpctl status
guard audio.log pactl info
guard audio.log aplay -l
guard audio.log arecord -l
guard audio.log systemctl status pipewire --no-pager -l
guard audio.log systemctl status wireplumber --no-pager -l

# Câmara: V4L2 + módulo.
guard video.log v4l2-ctl --list-devices
guard video.log ls -l /dev/video0
guard video.log lsmod
say video.log sh -c "dmesg | grep -iE 'uvc|video[0-9]|snd|sof|hda' | tail -n 120"

# Estado do produto (LocalAdmin via token no socket).
guard state.log nosctl doctor
guard state.log nosctl health
guard state.log nosctl config-show
guard state.log nosctl energy
say state.log sh -c "ls -l /var/lib/nosd /var/lib/nosd/models 2>&1"
for f in /tmp/nos-firstboot.log /var/lib/nosd/firstboot.log \
         /var/lib/nosd/hotswap.log /var/lib/nosd/jarbas-cage.log \
         /var/lib/nosd/wifi-diag.log /var/lib/nosd/boot-score.json; do
  [ -f "$f" ] && cp -f "$f" "$D/" 2>/dev/null || true
done
for f in /var/lib/nosd/firstboot.conf /var/lib/nosd/firstboot.done /var/lib/nosd/wifi.conf; do
  [ -f "$f" ] && cp -f "$f" "$D/" 2>/dev/null || true
done

sync
tar -C "$OUT" -czf "$OUT/diag-$STAMP.tar.gz" "diag-$STAMP" 2>/dev/null || true
sync
echo "OK: $D"
echo "    $OUT/diag-$STAMP.tar.gz"
echo "Copiar para o Windows e pôr em docs/evidence/hw/$STAMP/ (sem segredos)."
