#!/usr/bin/env python3
"""Write transmute-stage.nix with LF only. s521: deliver transmute on login (no GIS wait)."""
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "modules" / "transmute-stage.nix"

CONTENT = r'''# Estagio 0: GNOME (Wi-Fi/Settings) + ENTREGA garantida do nos-transmute + Hotswap JARBAS.
# s521: metal s520 so mostrou GNOME — GIS nao correu e o watcher esperava o wizard.
# Entrega: abrir transmute ~5s apos login (xdg autostart + systemd user + Desktop).
# Wi-Fi: GNOME Settings / nm-applet. GIS deixa de ser gate.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nixaiosTransmute;

  pickLogDirSh = ''
    LOGDIR=/var/lib/nosd
    if mountpoint -q /mnt/nixaios-log 2>/dev/null && [ -w /mnt/nixaios-log ]; then
      LOGDIR=/mnt/nixaios-log
    fi
  '';

  hotswapScript = pkgs.writeShellScript "nosd-hotswap-aios" ''
    set -euo pipefail
    ${pickLogDirSh}
    LOG="$LOGDIR/hotswap.log"
    mkdir -p "$LOGDIR" /var/lib/nosd
    exec > >(tee -a "$LOG") 2>&1
    echo "=== nosd-hotswap-aios start $(date -Is 2>/dev/null || echo now) ==="

    LOC=""
    for f in /var/lib/nosd/locale.conf /mnt/nixaios-log/nosd-persist/locale.conf /var/lib/AccountsService/users/nixos /etc/locale.conf; do
      if [ -f "$f" ]; then
        cand=$(grep -E '^(LANG|Language|FormatsLocale)=' "$f" 2>/dev/null | head -1 | cut -d= -f2- | tr -d "\"' " || true)
        if [ -n "$cand" ]; then LOC="$cand"; break; fi
      fi
    done
    [ -n "$LOC" ] || LOC="pt_BR.UTF-8"
    echo "LANG=$LOC" > /var/lib/nosd/locale.conf
    chown nosd:nosd /var/lib/nosd/locale.conf || true
    if mountpoint -q /mnt/nixaios-log 2>/dev/null; then
      mkdir -p /mnt/nixaios-log/nosd-persist
      cp -f /var/lib/nosd/locale.conf /mnt/nixaios-log/nosd-persist/locale.conf || true
    fi

    mkdir -p /etc/nixos
    cat > /etc/nixos/ai-managed.nix <<EOF
# Gerado por nos-transmute (Hotswap AIOS Full)
{ ... }: {
  i18n.defaultLocale = "$LOC";
  services.nosd.enable = true;
  services.jarbasCage.enable = true;
}
EOF
    echo "hotswap: wrote /etc/nixos/ai-managed.nix (locale=$LOC)"

    echo "hotswap: stopping display-manager (GDM / GNOME)…"
    systemctl stop display-manager.service 2>/dev/null || true
    pkill -f 'gnome-shell|gdm-wayland-session|Xwayland' 2>/dev/null || true
    for i in $(seq 1 30); do
      if [ -e /dev/dri/card0 ] || [ -e /dev/dri/card1 ]; then
        if ! ${pkgs.procps}/bin/pgrep -f 'gnome-shell|gdm' >/dev/null 2>&1; then
          echo "hotswap: DRM free (poll=$i)"
          break
        fi
      fi
      sleep 0.5
    done
    sleep 1

    echo "hotswap: restarting nosd…"
    systemctl restart nosd.service || true
    for i in $(seq 1 30); do
      if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then
        echo "hotswap: nosd HTTP OK"
        break
      fi
      sleep 0.5
    done

    echo "hotswap: starting jarbas-cage.service…"
    systemctl reset-failed jarbas-cage.service 2>/dev/null || true
    if ! systemctl start jarbas-cage.service; then
      echo "hotswap: FAIL systemctl start jarbas-cage" >&2
      mv -f /var/lib/nosd/hotswap-aios.request /var/lib/nosd/hotswap-aios.failed 2>/dev/null || true
      exit 1
    fi
    sleep 3
    if ! systemctl is-active --quiet jarbas-cage.service; then
      echo "hotswap: FAIL jarbas-cage not active after start" >&2
      systemctl status jarbas-cage.service --no-pager || true
      mv -f /var/lib/nosd/hotswap-aios.request /var/lib/nosd/hotswap-aios.failed 2>/dev/null || true
      exit 1
    fi
    ${pkgs.kbd}/bin/chvt 7 || true
    rm -f /var/lib/nosd/hotswap-aios.request
    echo "=== nosd-hotswap-aios DONE (jarbas-cage active, VT7) ==="
  '';

  revertGuiScript = pkgs.writeShellScript "nosd-revert-gui" ''
    set -euo pipefail
    rm -f /var/lib/nosd/aios-mode /mnt/nixaios-log/nosd-persist/aios-mode 2>/dev/null || true
    systemctl stop jarbas-cage.service 2>/dev/null || true
    systemctl start display-manager.service
  '';

  syncGnomeLocale = pkgs.writeShellScript "nos-sync-gnome-locale" ''
    set -euo pipefail
    LOC=""
    if [ -f /var/lib/AccountsService/users/nixos ]; then
      LOC=$(grep -E '^(Language|FormatsLocale)=' /var/lib/AccountsService/users/nixos 2>/dev/null | head -1 | cut -d= -f2- | tr -d "\"' " || true)
    fi
    if [ -z "$LOC" ] && command -v gsettings >/dev/null 2>&1; then
      reg=$(gsettings get org.gnome.system.locale region 2>/dev/null | tr -d "'\" " || true)
      case "$reg" in
        *.UTF-8|*_*) LOC="$reg" ;;
      esac
    fi
    if [ -z "$LOC" ] && [ -f /etc/locale.conf ]; then
      LOC=$(grep -E '^LANG=' /etc/locale.conf 2>/dev/null | head -1 | cut -d= -f2- | tr -d "\"' " || true)
    fi
    [ -n "$LOC" ] || LOC="pt_BR.UTF-8"
    mkdir -p "$HOME/.config" /var/lib/nosd
    echo "$LOC" > "$HOME/.config/gnome-initial-setup-locale"
    echo "LANG=$LOC" | ${pkgs.sudo}/bin/sudo tee /var/lib/nosd/locale.conf >/dev/null 2>&1 || \
      echo "LANG=$LOC" > /var/lib/nosd/locale.conf || true
    if mountpoint -q /mnt/nixaios-log 2>/dev/null; then
      mkdir -p /mnt/nixaios-log/nosd-persist
      echo "LANG=$LOC" > /mnt/nixaios-log/nosd-persist/locale.conf || true
    fi
  '';

  openTransmuteGui = pkgs.writeShellScript "nos-open-transmute-gui" ''
    set -euo pipefail
    ${pickLogDirSh}
    mkdir -p "$LOGDIR" /tmp/nos-transmute-chrome-profile
    FAILLOG="$LOGDIR/transmute-launch.log"
    {
      echo "=== nos-open-transmute-gui $(date -Is 2>/dev/null || echo now) uid=$(id -u) DISPLAY=''${DISPLAY:-} WAYLAND=''${WAYLAND_DISPLAY:-} ==="
      ${syncGnomeLocale} || true
    } >>"$FAILLOG" 2>&1

    OK=0
    for i in $(seq 1 90); do
      if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/transmute 2>/dev/null; then
        OK=1
        echo "nosd transmute HTTP 200 (try=$i)" >>"$FAILLOG"
        break
      fi
      ${pkgs.sudo}/bin/sudo systemctl start nosd.service 2>/dev/null || true
      sleep 1
    done
    if [ "$OK" != "1" ]; then
      echo "FAIL: nosd :7701/transmute nao respondeu em 90s" >>"$FAILLOG"
      # Fallback honesto: janela de erro via zenity/notify se existir
      if command -v zenity >/dev/null 2>&1; then
        zenity --error --title="Neural AIOS" --text="nosd offline (127.0.0.1:7701). Ver $FAILLOG" || true
      fi
      exit 1
    fi

    echo "launching chromium transmute UI" >>"$FAILLOG"
    exec ${pkgs.chromium}/bin/chromium \
      --user-data-dir=/tmp/nos-transmute-chrome-profile \
      --no-first-run \
      --no-default-browser-check \
      --disable-session-crashed-bubble \
      --disable-infobars \
      --start-maximized \
      --app="http://127.0.0.1:7701/transmute"
  '';

  # s521: NAO esperar GIS. Entregar transmute ~5s apos a sessao grafica.
  sessionDeliverTransmute = pkgs.writeShellScript "nos-session-deliver-transmute" ''
    set -euo pipefail
    ${pickLogDirSh}
    mkdir -p "$LOGDIR"
    WLOG="$LOGDIR/wizard-watch.log"
    echo "=== nos-session-deliver-transmute start $(date -Is 2>/dev/null || echo now) uid=$(id -u) ===" >>"$WLOG"

    if [ -f /mnt/nixaios-log/nosd-persist/aios-mode ]; then
      mode=$(tr -d ' \n\r' < /mnt/nixaios-log/nosd-persist/aios-mode || true)
      if [ "$mode" = "aios" ]; then
        echo "aios-mode set — skip (hotswap owns UI)" >>"$WLOG"
        exit 0
      fi
    fi

    # Evitar 2 Chromiums se xdg + systemd user dispararem juntos
    LOCK=/tmp/nos-transmute-deliver.lock
    if [ -f "$LOCK" ]; then
      age=$(( $(date +%s) - $(stat -c %Y "$LOCK" 2>/dev/null || echo 0) ))
      if [ "$age" -lt 120 ]; then
        echo "skip: deliver ja em curso (lock age=''${age}s)" >>"$WLOG"
        exit 0
      fi
    fi
    echo $$ > "$LOCK"

    echo "waiting 5s for gnome-shell…" >>"$WLOG"
    sleep 5
    exec ${openTransmuteGui}
  '';

  transmuteCli = pkgs.writeShellScriptBin "nos-transmute" ''
    set -euo pipefail
    CMD="''${1:-gui}"
    case "$CMD" in
      gui)
        exec ${openTransmuteGui}
        ;;
      status)
        exec nosctl --socket /run/nosd/nosd.sock transmute-status
        ;;
      run|auto)
        shift || true
        exec nosctl --socket /run/nosd/nosd.sock transmute "$@"
        ;;
      hotswap)
        exec nosctl --socket /run/nosd/nosd.sock transmute --no-download
        ;;
      revert-gui)
        exec sudo ${revertGuiScript}
        ;;
      *)
        echo "Uso: nos-transmute [gui | status | run | hotswap | revert-gui]"
        exit 1
        ;;
    esac
  '';

  desktopEntry = ''
    [Desktop Entry]
    Type=Application
    Name=Transmutar para Neural AIOS
    Comment=Instalar AIOS Full (MoE + STT + TTS) e Hotswap JARBAS
    Exec=${openTransmuteGui}
    Icon=system-software-install
    Terminal=false
    Categories=System;Settings;
  '';
in
{
  options.services.nixaiosTransmute = {
    enable = lib.mkEnableOption "GNOME + nos-transmute entrega garantida + Hotswap JARBAS";
  };

  config = lib.mkIf cfg.enable {
    time.timeZone = lib.mkDefault "America/Sao_Paulo";
    i18n.defaultLocale = lib.mkDefault "pt_BR.UTF-8";
    i18n.supportedLocales = lib.mkDefault [
      "pt_BR.UTF-8/UTF-8"
      "pt_PT.UTF-8/UTF-8"
      "en_US.UTF-8/UTF-8"
      "es_ES.UTF-8/UTF-8"
      "fr_FR.UTF-8/UTF-8"
      "de_DE.UTF-8/UTF-8"
    ];

    # GNOME desktop (Wi-Fi via Settings). Sem kernel GIS force — nao entregou no metal s520.
    services.xserver.enable = lib.mkForce true;
    services.xserver.xkb.layout = lib.mkDefault "br";
    services.displayManager.gdm = {
      enable = true;
      autoSuspend = false;
    };
    services.desktopManager.gnome.enable = true;
    services.displayManager.autoLogin = {
      enable = true;
      user = "nixos";
    };
    services.xserver.displayManager.gdm.enable = lib.mkDefault true;
    services.xserver.desktopManager.gnome.enable = lib.mkDefault true;

    services.getty.autologinUser = lib.mkForce null;

    # Atalho no dock / favoritos GNOME
    programs.dconf.enable = true;
    programs.dconf.profiles.user.databases = [
      {
        settings."org/gnome/shell" = {
          favorite-apps = [
            "nos-transmute.desktop"
            "org.gnome.Settings.desktop"
            "org.gnome.Nautilus.desktop"
            "org.gnome.Terminal.desktop"
          ];
        };
      }
    ];

    environment.gnome.excludePackages = with pkgs; [
      epiphany
      geary
      totem
      gnome-music
      gnome-tour
    ];

    security.sudo.wheelNeedsPassword = false;
    services.jarbasCage.autoStart = lib.mkForce false;

    environment.systemPackages = with pkgs; [
      transmuteCli
      llama-cpp
      whisper-cpp
      espeak-ng
      alsa-utils
      pavucontrol
      networkmanagerapplet
      zenity
      gnome-terminal
    ];

    # 1) XDG autostart — delay curto, SEM esperar wizard
    environment.etc."xdg/autostart/nos-transmute-deliver.desktop".text = ''
      [Desktop Entry]
      Type=Application
      Name=Neural AIOS — Transmutacao
      Comment=Abre o conversor AIOS ao entrar no GNOME
      Exec=${sessionDeliverTransmute}
      Terminal=false
      X-GNOME-Autostart-enabled=true
      X-GNOME-Autostart-Delay=3
      X-GNOME-Autostart-Phase=Application
    '';

    # nm-applet para Wi-Fi se Settings nao for o caminho do user
    environment.etc."xdg/autostart/nm-applet.desktop".text = ''
      [Desktop Entry]
      Type=Application
      Name=Network Manager Applet
      Exec=${pkgs.networkmanagerapplet}/bin/nm-applet
      Terminal=false
      X-GNOME-Autostart-enabled=true
    '';

    environment.etc."xdg/applications/nos-transmute.desktop".text = desktopEntry;

    # 2) systemd --user: mais fiavel que so xdg em algumas sessoes GNOME/NixOS
    systemd.user.services.nos-transmute-deliver = {
      description = "Deliver nos-transmute UI after graphical session";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${sessionDeliverTransmute}";
        Restart = "on-failure";
        RestartSec = "8s";
      };
    };

    # Linger para o user live poder ter user units cedo
    users.users.nixos.linger = true;

    # 3) Desktop + ~/.config/autostart (copia sem indentacao invalida)
    system.activationScripts.nosTransmuteDesktop = ''
      mkdir -p /home/nixos/Desktop /home/nixos/.config/autostart
      cat > /home/nixos/Desktop/Transmutar-Neural-AIOS.desktop << 'EOF'
${desktopEntry}EOF
      cat > /home/nixos/.config/autostart/nos-transmute-deliver.desktop << EOF
[Desktop Entry]
Type=Application
Name=Neural AIOS — Transmutacao
Exec=${sessionDeliverTransmute}
Terminal=false
X-GNOME-Autostart-enabled=true
X-GNOME-Autostart-Delay=3
EOF
      chmod +x /home/nixos/Desktop/Transmutar-Neural-AIOS.desktop || true
      chown -R nixos:users /home/nixos/Desktop /home/nixos/.config 2>/dev/null || true
    '';

    systemd.paths.nosd-hotswap-aios = {
      wantedBy = [ "multi-user.target" "graphical.target" ];
      pathConfig = {
        PathExists = "/var/lib/nosd/hotswap-aios.request";
        Unit = "nosd-hotswap-aios.service";
      };
    };

    systemd.services.nosd-hotswap-aios = {
      description = "Hotswap GNOME → JARBAS AIOS Kiosk";
      after = [ "nosd.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${hotswapScript}";
      };
    };

    systemd.services.nosd-boot-persist-check = {
      description = "Auto-enter JARBAS if persistent USB is already transmuted";
      wantedBy = [ "multi-user.target" ];
      after = [ "nosd.service" "mnt-nixaios\\x2dlog.mount" "display-manager.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "nosd-boot-persist-check" ''
          sleep 3
          if [ -f /mnt/nixaios-log/nosd-persist/aios-mode ]; then
            mode=$(tr -d ' \n\r' < /mnt/nixaios-log/nosd-persist/aios-mode || true)
            if [ "$mode" = "aios" ]; then
              cp -f /mnt/nixaios-log/nosd-persist/locale.conf /var/lib/nosd/locale.conf 2>/dev/null || true
              touch /var/lib/nosd/hotswap-aios.request
              systemctl start --no-block nosd-hotswap-aios.service || true
            fi
          fi
        '';
      };
    };
  };
}
'''

def main() -> None:
    text = CONTENT.replace("\r\n", "\n").replace("\r", "\n")
    # Fix desktopEntry embedding in activation: desktopEntry lines must not break Nix
    OUT.write_bytes(text.encode("utf-8"))
    raw = OUT.read_bytes()
    assert b"\r" not in raw
    assert b"sessionDeliverTransmute" in raw
    assert b"sevices" not in raw
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
