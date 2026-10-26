#!/usr/bin/env bash
# Hotswap AIOS Full — usado por modules/transmute-stage.nix (0.5.2).
# Placeholders: @curl@ @kbd@ @nosd@
set -euo pipefail
LOGDIR=/var/lib/nosd
mkdir -p "$LOGDIR" 2>/dev/null || true
LOG="$LOGDIR/hotswap.log"
mkdir -p "$LOGDIR" /var/lib/nosd
exec > >(tee -a "$LOG") 2>&1
echo "=== nosd-hotswap-aios start $(date -Is 2>/dev/null || echo now) ==="

LOC=""
for f in /var/lib/nosd/locale.conf /etc/locale.conf; do
  if [ -f "$f" ]; then
    cand=$(grep -E '^LANG=' "$f" 2>/dev/null | head -1 | cut -d= -f2- | tr -d "\"' " || true)
    if [ -n "$cand" ]; then LOC="$cand"; break; fi
  fi
done
[ -n "$LOC" ] || LOC="pt_BR.UTF-8"
echo "LANG=$LOC" > /var/lib/nosd/locale.conf
chown nosd:nosd /var/lib/nosd/locale.conf || true
chmod a+rw /var/lib/nosd/locale.conf 2>/dev/null || true

mkdir -p /etc/nixos
cat > /etc/nixos/ai-managed.nix <<EOF
# Gerado por nos-firstboot / nos-transmute (Hotswap AIOS Full)
{ ... }: {
  i18n.defaultLocale = "$LOC";
  services.nosd.enable = true;
  services.jarbasCage.enable = true;
}
EOF
echo "hotswap: wrote /etc/nixos/ai-managed.nix (locale=$LOC)"

echo "hotswap: stopping display-manager (se existir)…"
systemctl stop display-manager.service 2>/dev/null || true
pkill -f 'gnome-shell|gdm-wayland-session|Xwayland' 2>/dev/null || true
sleep 1

echo "hotswap: restarting nosd…"
systemctl restart nosd.service || true
for i in $(seq 1 30); do
  if @nosd@/bin/nosctl --socket /run/nosd/nosd.sock health >/dev/null 2>&1; then
    echo "hotswap: nosd health OK"
    break
  fi
  sleep 0.5
done

echo "hotswap: starting jarbas-cage.service…"
systemctl reset-failed jarbas-cage.service seatd.service 2>/dev/null || true
if [ -S /run/seatd.sock ] && ! systemctl is-active --quiet seatd.service 2>/dev/null; then
  rm -f /run/seatd.sock
fi
systemctl start seatd.service 2>/dev/null || true
if ! systemctl start jarbas-cage.service; then
  echo "hotswap: FAIL systemctl start jarbas-cage" >&2
  journalctl -u jarbas-cage -u seatd -n 40 --no-pager || true
  mv -f /var/lib/nosd/hotswap-aios.request /var/lib/nosd/hotswap-aios.failed 2>/dev/null || true
  exit 1
fi
sleep 5
if ! systemctl is-active --quiet jarbas-cage.service; then
  echo "hotswap: FAIL jarbas-cage not active after start" >&2
  systemctl status jarbas-cage.service --no-pager || true
  journalctl -u jarbas-cage -u seatd -n 40 --no-pager || true
  mv -f /var/lib/nosd/hotswap-aios.request /var/lib/nosd/hotswap-aios.failed 2>/dev/null || true
  exit 1
fi
@kbd@/bin/chvt 7 || true
rm -f /var/lib/nosd/hotswap-aios.request
echo "=== nosd-hotswap-aios DONE (jarbas-cage active, VT7) ==="
