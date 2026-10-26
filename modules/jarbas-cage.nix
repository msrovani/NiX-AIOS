# JARBAS Wayland kiosk — ADR-0118
# s527/s528: default Slint HUD (iGPU cage); Chromium = opt-in cinematic.
# dGPU llama-server ≠ este unit — ver services.nosdEngine (nosd-engine@.service).
# s520: conflicts display-manager (hotswap DRM).
{ config, lib, pkgs, ... }:
let
  cfg = config.services.jarbasCage;

  waitScript = pkgs.writeShellScript "jarbas-wait-deps" ''
    set -euo pipefail
    # Root ExecStartPre (+): runtime dirs + seatd sock stale
    mkdir -p /run/user/nosd /var/lib/nosd
    chown nosd:nosd /run/user/nosd /var/lib/nosd || true
    chmod 700 /run/user/nosd || true
    chmod 750 /var/lib/nosd || true
    touch /var/lib/nosd/jarbas-cage.log
    chown nosd:nosd /var/lib/nosd/jarbas-cage.log || true
    # Stale seatd.sock (crash) bloqueia seatd.service / seatd-launch
    if [ -S /run/seatd.sock ] && ! systemctl is-active --quiet seatd.service 2>/dev/null; then
      echo "jarbas-wait: removing stale /run/seatd.sock"
      rm -f /run/seatd.sock
    fi
    for i in $(seq 1 60); do
      if [ -S /run/nosd/nosd.sock ]; then break; fi
      sleep 0.5
    done
    for i in $(seq 1 40); do
      if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then break; fi
      sleep 0.5
    done
  '';

  pickDrm = pkgs.writeShellScript "jarbas-pick-drm" ''
    set -euo pipefail
    prefer="${cfg.drmDevice}"
    if [ -n "$prefer" ] && [ -e "$prefer" ]; then
      echo "$prefer"; exit 0
    fi
    for c in /dev/dri/card*; do
      [ -e "$c" ] || continue
      base=$(basename "$c")
      sys="/sys/class/drm/$base/device/vendor"
      [ -r "$sys" ] || continue
      v=$(cat "$sys" 2>/dev/null || true)
      if [ "$v" = "0x8086" ]; then echo "$c"; exit 0; fi
    done
    for c in /dev/dri/card*; do
      [ -e "$c" ] || continue
      base=$(basename "$c")
      sys="/sys/class/drm/$base/device/vendor"
      v=$(cat "$sys" 2>/dev/null || true)
      if [ "$v" = "0x10de" ]; then continue; fi
      echo "$c"; exit 0
    done
    for c in /dev/dri/card*; do
      if [ -e "$c" ]; then echo "$c"; exit 0; fi
    done
    echo ""
  '';

  cinematicWrap = pkgs.writeShellScript "jarbas-cinematic-wrap" ''
    set -euo pipefail
    CURL="${pkgs.curl}/bin/curl"
    # User=nosd: NUNCA /run/*.log (root-only) — tee Permission denied derruba o unit (metal s529).
    # s529: só /var/lib/nosd (FAT OFF)
    LOG=/var/lib/nosd/jarbas-cage.log
    touch "$LOG" 2>/dev/null || LOG=/run/user/nosd/jarbas-cage.log
    touch "$LOG" 2>/dev/null || LOG=/tmp/jarbas-cage-$UID.log
    exec > >(tee -a "$LOG") 2>&1
    echo "=== jarbas-cinematic start $(date -Is 2>/dev/null || echo now) uid=$(id -u) log=$LOG ==="
    export XDG_RUNTIME_DIR=/run/user/nosd
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=cage
    export NOSD_SOCKET=/run/nosd/nosd.sock
    export RUST_LOG="''${RUST_LOG:-info}"
    export PATH="${pkgs.curl}/bin:${pkgs.cage}/bin:${pkgs.seatd}/bin:$PATH"
    mkdir -p "$XDG_RUNTIME_DIR"
    chmod 700 "$XDG_RUNTIME_DIR" || true

    DRM="$(${pickDrm})"
    if [ -z "$DRM" ]; then
      echo "jarbas-cinematic: sem DRM" >&2
      exit 1
    fi
    export WLR_DRM_DEVICES="$DRM"
    export WLR_NO_HARDWARE_CURSORS=1
    unset LIBGL_ALWAYS_SOFTWARE || true
    echo "jarbas-cinematic: DRM=$DRM mode=${cfg.uiMode}"
    ls -la /dev/dri 2>/dev/null || true
    $CURL -sf -o /dev/null http://127.0.0.1:7701/ && echo "orb_lab: OK" || echo "orb_lab: DOWN"

    LAB="http://127.0.0.1:7701/"

    if [ "${cfg.uiMode}" = "slint" ]; then
      export FONTCONFIG_FILE="${pkgs.fontconfig.out}/etc/fonts/fonts.conf"
      export SLINT_DEFAULT_FONT="${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf"
      export LIBSEAT_BACKEND=seatd
      CAGE_BIN="${pkgs.cage}/bin/cage"
      SEAT_WRAP="${pkgs.seatd}/bin/seatd-launch"
      # Preferir seatd.service já activo (After=seatd). seatd-launch em cima = "Socket file found" (metal).
      run_cage() {
        if [ -S /run/seatd.sock ]; then
          echo "jarbas-slint: using system seatd.sock"
          "$CAGE_BIN" -s -- "$@"
        elif [ -x "$SEAT_WRAP" ]; then
          echo "jarbas-slint: seatd-launch (no system sock)"
          "$SEAT_WRAP" -- "$CAGE_BIN" -s -- "$@"
        else
          "$CAGE_BIN" -s -- "$@"
        fi
      }
      run_slint() {
        echo "jarbas-slint: DRM=$WLR_DRM_DEVICES soft=''${JARBAS_FORCE_SOFTWARE:-0}"
        run_cage ${cfg.nosdPackage}/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock --interval-secs 2
      }
      # 1) GL/iGPU  2) software fallback (evita loop mudo se GPU falhar)
      unset SLINT_BACKEND || true
      unset LIBGL_ALWAYS_SOFTWARE || true
      if run_slint; then exit 0; fi
      echo "jarbas-slint: GL/cage falhou — retry winit-software" >&2
      export JARBAS_FORCE_SOFTWARE=1
      export SLINT_BACKEND=winit-software
      export LIBGL_ALWAYS_SOFTWARE=1
      if [ -S /run/seatd.sock ]; then
        exec "$CAGE_BIN" -s -- \
          ${cfg.nosdPackage}/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock --interval-secs 2
      elif [ -x "$SEAT_WRAP" ]; then
        exec "$SEAT_WRAP" -- "$CAGE_BIN" -s -- \
          ${cfg.nosdPackage}/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock --interval-secs 2
      else
        exec "$CAGE_BIN" -s -- \
          ${cfg.nosdPackage}/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock --interval-secs 2
      fi
    fi

    if [ ! -S "$XDG_RUNTIME_DIR/pipewire-0" ]; then
      ${pkgs.pipewire}/bin/pipewire &
      ${pkgs.wireplumber}/bin/wireplumber &
      sleep 1
    fi

    mkdir -p /run/user/nosd/chromium-jarbas
    echo "jarbas-cinematic: launching chromium kiosk $LAB"
    exec ${pkgs.cage}/bin/cage -s -- ${pkgs.chromium}/bin/chromium \
      --kiosk \
      --noerrdialogs \
      --disable-infobars \
      --disable-session-crashed-bubble \
      --no-first-run \
      --check-for-update-interval=31536000 \
      --autoplay-policy=no-user-gesture-required \
      --use-fake-ui-for-media-stream \
      --enable-features=WebRTCPipeWireCapturer \
      --enable-gpu \
      --ignore-gpu-blocklist \
      --user-data-dir=/run/user/nosd/chromium-jarbas \
      "$LAB"
  '';

  switchVt = pkgs.writeShellScript "jarbas-chvt" ''
    set -euo pipefail
    sleep 2
    ${pkgs.kbd}/bin/chvt "$(echo ${cfg.tty} | tr -dc '0-9')" || true
    MODE="${cfg.uiMode}"
    printf '\n*** JARBAS %s = %s — Alt+F%s ***\n' "$MODE" "${cfg.tty}" "$(echo ${cfg.tty} | tr -dc '0-9')" > /dev/tty1 || true
  '';
in
{
  options.services.jarbasCage = {
    enable = lib.mkEnableOption "JARBAS HUD kiosk (cage)";
    nosdPackage = lib.mkOption {
      type = lib.types.package;
      default = pkgs.nosd or pkgs.hello;
    };
    drmDevice = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "Pin WLR_DRM_DEVICES. Vazio = prefer Intel iGPU.";
    };
    tty = lib.mkOption {
      type = lib.types.str;
      default = "tty7";
    };
    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = true;
    };
    hubFallback = lib.mkOption {
      type = lib.types.bool;
      default = true;
    };
    uiMode = lib.mkOption {
      type = lib.types.enum [ "cinematic" "slint" ];
      default = "slint";
      description = "slint = HUD nativo (default s527); cinematic = Chromium orb lab.";
    };
  };

  config = lib.mkIf cfg.enable {
    services.seatd.enable = true;
    services.xserver.enable = lib.mkDefault false;

    services.pipewire = {
      enable = true;
      alsa.enable = true;
      pulse.enable = true;
      wireplumber.enable = true;
    };
    security.rtkit.enable = true;

    users.users.nosd.extraGroups = lib.mkAfter [
      "video"
      "render"
      "seat"
      "audio"
      "input"
    ];

    fonts.packages = with pkgs; [ dejavu_fonts ];

    systemd.tmpfiles.rules = [
      "d /run/user/nosd 0700 nosd nosd -"
      "d /run/user/nosd/chromium-jarbas 0700 nosd nosd -"
    ];

    environment.systemPackages = with pkgs; [
      cage
      cfg.nosdPackage
      chromium
      curl
      v4l-utils
      mesa-demos
    ];

    services.udev.extraRules = ''
      KERNEL=="video[0-9]*", GROUP="video", MODE="0660"
      KERNEL=="media[0-9]*", GROUP="video", MODE="0660"
    '';

    systemd.services.jarbas-hub = lib.mkIf cfg.hubFallback {
      description = "JARBAS hub slog (sem DRM)";
      wantedBy = [ "multi-user.target" ];
      after = [ "nosd.service" ];
      wants = [ "nosd.service" ];
      serviceConfig = {
        Type = "simple";
        User = "nosd";
        Group = "nosd";
        ExecStartPre = "+${waitScript}";
        ExecStart = "${cfg.nosdPackage}/bin/nos-jarbas --socket /run/nosd/nosd.sock --interval-secs 2";
        Restart = "on-failure";
        RestartSec = "5s";
        Environment = [ "NOSD_SOCKET=/run/nosd/nosd.sock" "RUST_LOG=info" ];
      };
    };

    systemd.services.jarbas-cage = {
      description = "JARBAS kiosk (${cfg.uiMode} / iGPU cage)";
      wantedBy = lib.mkIf cfg.autoStart [ "multi-user.target" ];
      after = [
        "nosd.service"
        "nosd-early-score.service"
        "seatd.service"
        "jarbas-hub.service"
        "getty@tty1.service"
      ] ++ lib.optionals (cfg.uiMode == "cinematic") [
        "pipewire.service"
        "wireplumber.service"
      ];
      wants = [ "nosd.service" "seatd.service" "nosd-early-score.service" ];
      conflicts = [ "getty@${cfg.tty}.service" "display-manager.service" ];
      serviceConfig = {
        Type = "simple";
        User = "nosd";
        Group = "nosd";
        TTYPath = "/dev/${cfg.tty}";
        TTYReset = "yes";
        TTYVHangup = "yes";
        StandardInput = "tty";
        StandardOutput = "journal";
        StandardError = "journal";
        ExecStartPre = "+${waitScript}";
        ExecStart = "${cinematicWrap}";
        # Anti-loop: Chromium reiniciava em spam (prompt+mouse sem UI estável)
        Restart = "on-failure";
        RestartSec = "10s";
        StartLimitIntervalSec = 180;
        StartLimitBurst = 3;
        TimeoutStopSec = "20s";
        # seat = libseat/seatd; sem isto DRM master falha mesmo com DeviceAllow.
        SupplementaryGroups = "video audio render input seat";
        # Confinamento host thin (I.3): Preferir cage vivo a ProtectSystem=strict
        # (quebra seatd/DRM no metal — lição PAMName/seat). Landlock syscall = nosd bin.
        ProtectHome = true;
        NoNewPrivileges = true;
        DeviceAllow = [ "char-drm rw" "char-input rw" ];
        OOMScoreAdjust = -500;
        # PATH: wrap usa paths absolutos; curl bare no PATH evita regressão
        Environment = [
          "XDG_RUNTIME_DIR=/run/user/nosd"
          "NOSD_SOCKET=/run/nosd/nosd.sock"
          "RUST_LOG=info,jarbas=info"
          "PATH=${pkgs.curl}/bin:${pkgs.coreutils}/bin:${pkgs.cage}/bin:${pkgs.seatd}/bin"
        ];
      };
    };

    systemd.services.jarbas-show-vt = lib.mkIf cfg.autoStart {
      description = "JARBAS chvt -> UI tty";
      wantedBy = [ "multi-user.target" ];
      after = [ "jarbas-cage.service" ];
      wants = [ "jarbas-cage.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "+${switchVt}";
        RemainAfterExit = true;
      };
    };
  };
}
