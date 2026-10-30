# JARBAS Wayland kiosk � ADR-0118
# 0.5.x: default Slint HUD (iGPU cage); Chromium = opt-in cinematic.
# dGPU llama-server ? este unit � ver services.nosdEngine (nosd-engine@.service).
# 0.5.x: conflicts display-manager (hotswap DRM).
{ config, lib, pkgs, ... }:
let
  cfg = config.services.jarbasCage;

  chooseMode = pkgs.writeShellScript "jarbas-select-ui-mode" ''
    if [ "${cfg.uiMode}" != "auto" ]; then
      echo "${cfg.uiMode}"
      exit 0
    fi
    available_kib="$(${pkgs.gawk}/bin/awk '/^MemAvailable:/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
    available_mib=$((available_kib / 1024))
    if [ "${if cfg.chromiumGpuValidated then "1" else "0"}" = "1" ] \
      && [ "$available_mib" -ge "${toString cfg.chromiumMinAvailableMiB}" ]; then
      echo cinematic
    else
      echo slint
    fi
  '';

  waitScript = pkgs.writeShellScript "jarbas-wait-deps" ''
    set -euo pipefail
    # Root ExecStartPre (+): runtime dirs + seatd sock stale
    # 0.5.0: NAO apertar 750 � ISO live precisa 1777 para firstboot+nosd (EACCES p�s-Q5).
    # 0.6: XDG_RUNTIME_DIR em /run/nosd/user (NAO /run/user/nosd).
    # /run/user e tmpfs montado pelo logind no 1o login e ESCONDE o que
    # tmpfiles/ExecStartPre criou antes (cage: mkdir /run/user EACCES).
    mkdir -p /run/nosd/user /var/lib/nosd/models /var/lib/nosd/nsgdb /var/lib/nosd/cache
    chown nosd:nosd /var/lib/nosd/cache || true
    chown nosd:nosd /run/nosd/user || true
    chown -R nosd:nosd /var/lib/nosd 2>/dev/null || chown nosd:nosd /var/lib/nosd || true
    chmod 700 /run/nosd/user || true
    # T0.7: 1777 só pré-transmute (live ISO: firstboot/user escreve marcadores).
    # Em aios-mode (pós-transmute) o data-dir volta ao sandbox 0750 nosd —
    # os marcadores que disparam serviços root (ex.: hotswap-aios.request)
    # deixam de ser forjáveis por qualquer user local.
    if [[ -f /var/lib/nosd/aios-mode ]]; then
      chmod 750 /var/lib/nosd 2>/dev/null || true
      chmod 755 /var/lib/nosd/models /var/lib/nosd/nsgdb 2>/dev/null || true
    else
      chmod 1777 /var/lib/nosd /var/lib/nosd/models /var/lib/nosd/nsgdb 2>/dev/null || true
    fi
    touch /var/lib/nosd/jarbas-cage.log
    chown nosd:nosd /var/lib/nosd/jarbas-cage.log || true
    chmod a+rw /var/lib/nosd/jarbas-cage.log 2>/dev/null || true
    # wlroots/seatd só faz page-flip na VT activa. Se o cage arranca com a
    # VT1 em foreground (firstboot), o 1o arranque fica PRETO e só um restart
    # resolve. Tornar a VT do kiosk activa ANTES do ExecStart — **mas só
    # quando o GUI foi pedido**: o `chvt` incondicional saltava do tty1 (onde
    # o questionário Q1–Q5 espera input) para o tty7, e as perguntas ficavam
    # invisíveis no metal. `gui-start` é tocado pelo hotswap (pós-Q5) e pelo
    # persist-check (2.º boot).
    GUI_START=/var/lib/nosd/gui-start
    FIRSTBOOT_DONE=/var/lib/nosd/firstboot.done
    AIOS_MODE=$(tr -d ' \n\r' < /var/lib/nosd/aios-mode 2>/dev/null || true)
    if [[ -f "$GUI_START" || -f "$FIRSTBOOT_DONE" || "$AIOS_MODE" = "aios" ]]; then
      ${pkgs.kbd}/bin/chvt "$(printf %s ${cfg.tty} | tr -dc 0-9)" 2>/dev/null || true
    fi
    sleep 0.3
    # Stale seatd.sock (crash) bloqueia seatd.service / seatd-launch
    if [ -S /run/seatd.sock ] && ! ${pkgs.systemd}/bin/systemctl is-active --quiet seatd.service 2>/dev/null; then
      echo "jarbas-wait: removing stale /run/seatd.sock"
      rm -f /run/seatd.sock
    fi
    for i in $(seq 1 60); do
      if [ -S /run/nosd/nosd.sock ]; then break; fi
      sleep 0.5
    done
    # O selector aplica o mesmo orçamento antes do launcher escolher renderer.
    if [ "$( ${chooseMode} )" = "cinematic" ]; then
      for i in $(seq 1 40); do
        if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then break; fi
        sleep 0.5
      done
    fi
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
    # User=nosd: NUNCA /run/*.log (root-only) � tee Permission denied derruba o unit (metal s529).
    # s529: s� /var/lib/nosd (FAT OFF)
    LOG=/var/lib/nosd/jarbas-cage.log
    : >>"$LOG" 2>/dev/null || LOG=/tmp/jarbas-cage-$UID.log
    if : >>"$LOG" 2>/dev/null; then exec >>"$LOG" 2>&1; fi
    trap 'rc=$?; echo "=== jarbas exit rc=$rc ==="' EXIT
    echo "=== jarbas-cinematic start $(date -Is 2>/dev/null || echo now) uid=$(id -u) log=$LOG ==="
    export XDG_RUNTIME_DIR=/run/nosd/user
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=cage
    export NOSD_SOCKET=/run/nosd/nosd.sock
    # Fontconfig: HOME=/var/empty -> "No writable cache directories" a cada arranque.
    export XDG_CACHE_HOME=/var/lib/nosd/cache
    export RUST_LOG="''${RUST_LOG:-info}"
    export PATH="${pkgs.curl}/bin:${pkgs.cage}/bin:${pkgs.seatd}/bin:$PATH"
    UI_MODE="$( ${chooseMode} )"
    if ! mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null; then
      echo "jarbas: XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR nao criavel; fallback /var/lib/nosd/run" >&2
      XDG_RUNTIME_DIR=/var/lib/nosd/run
      export XDG_RUNTIME_DIR
      mkdir -p "$XDG_RUNTIME_DIR"
    fi
    chmod 700 "$XDG_RUNTIME_DIR" || true

    DRM="$(${pickDrm})"
    if [ -z "$DRM" ]; then
      echo "jarbas-cinematic: sem DRM" >&2
      exit 1
    fi
    export WLR_DRM_DEVICES="$DRM"
    export WLR_NO_HARDWARE_CURSORS=1
    unset LIBGL_ALWAYS_SOFTWARE || true
    echo "jarbas: DRM=$DRM selected_mode=$UI_MODE configured_mode=${cfg.uiMode}"
    echo "jarbas: XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR ($(stat -c %U:%G:%a "$XDG_RUNTIME_DIR" 2>/dev/null || echo missing))"
    echo "jarbas: seatd sock=$( [ -S /run/seatd.sock ] && echo yes || echo no ) seatd=$(${pkgs.systemd}/bin/systemctl is-active seatd.service 2>/dev/null || true)"
    ls -la /dev/dri 2>/dev/null || true
    $CURL -sf -o /dev/null http://127.0.0.1:7701/ && echo "orb_lab: OK" || echo "orb_lab: DOWN"

    LAB="http://127.0.0.1:7701/"
    export FONTCONFIG_FILE="${pkgs.fontconfig.out}/etc/fonts/fonts.conf"
    export SLINT_DEFAULT_FONT="${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf"
    export LIBSEAT_BACKEND=seatd
    # winit escolhe X11 se WAYLAND_DISPLAY estiver vazio -> Xwayland ->
    # glamor/kms_swrast falha e a HUD morre. Forcar Wayland.
    export WINIT_UNIX_BACKEND=wayland
    CAGE_BIN="${pkgs.cage}/bin/cage"
    SEAT_WRAP="${pkgs.seatd}/bin/seatd-launch"
    # Preferir seatd.service ja activo (After=seatd); seatd-launch por cima do
    # sock do sistema da "Socket file found" no metal.
    run_cage() {
      if [ -S /run/seatd.sock ]; then
        echo "jarbas: using system seatd.sock"
        "$CAGE_BIN" -s -- "$@"
      elif [ -x "$SEAT_WRAP" ]; then
        echo "jarbas: seatd-launch (no system sock)"
        "$SEAT_WRAP" -- "$CAGE_BIN" -s -- "$@"
      else
        "$CAGE_BIN" -s -- "$@"
      fi
    }
    # WAYLAND_DISPLAY SO no filho: se estiver no env do cage, o cage arranca
    # em modo aninhado (cliente Wayland) em vez de DRM/KMS.
    run_slint() {
      run_cage ${pkgs.coreutils}/bin/env WINIT_UNIX_BACKEND=wayland "WAYLAND_DISPLAY=''${WAYLAND_DISPLAY:-wayland-0}" ${cfg.nosdPackage}/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock --interval-secs 2
    }
    # 1) GL/iGPU (metal)  2) software + pixman (QEMU/no-GL).
    # NAO usar LIBGL_ALWAYS_SOFTWARE: quebra o EGL do wlroots (kms_swrast).
    run_slint_software() {
      export JARBAS_FORCE_SOFTWARE=1
      export SLINT_BACKEND=winit-software
      export WLR_RENDERER=pixman
      unset LIBGL_ALWAYS_SOFTWARE || true
      run_slint
    }

    if [ "$UI_MODE" = "slint" ]; then
      unset SLINT_BACKEND || true
      unset LIBGL_ALWAYS_SOFTWARE || true
      if run_slint; then exit 0; fi
      echo "jarbas-slint: GL/cage falhou - retry winit-software + WLR_RENDERER=pixman" >&2
      run_slint_software
      exit $?
    fi

    # cinematic/Chromium: pipewire para mic/cam do kiosk.
    if [ ! -S "$XDG_RUNTIME_DIR/pipewire-0" ]; then
      ${pkgs.pipewire}/bin/pipewire &
      ${pkgs.wireplumber}/bin/wireplumber &
      sleep 1
    fi

    fail_over_to_slint() {
      echo "jarbas-chromium: $1; fail over para Slint" >&2
      run_slint_software
      exit $?
    }
    mkdir -p "$XDG_RUNTIME_DIR/chromium-jarbas"
    if ! $CURL -sf -o /dev/null "$LAB"; then
      fail_over_to_slint "lab indisponivel"
    fi
    # Chave bootstrap (0600): o daemon troca-a por cookie HttpOnly no
    # #bootstrap=... da URL (o fragmento nao viaja na rede) e o browser
    # re-troca-a se o nosd reiniciar. O token IPC NUNCA entra no HTML/JS.
    BOOTSTRAP="$(${pkgs.coreutils}/bin/cat /proc/sys/kernel/random/uuid | ${pkgs.coreutils}/bin/tr -d '-')$(${pkgs.coreutils}/bin/cat /proc/sys/kernel/random/uuid | ${pkgs.coreutils}/bin/tr -d '-')"
    BOOTSTRAP_FILE="${config.services.nosd.dataDir}/jarbas-ui-bootstrap.token"
    # data-dir e 1777 (live ISO): um user local pode ja ter plantado o caminho.
    # install falha no chmod se o dono nao e nosd -> nao promover a Chromium.
    if ! ${pkgs.coreutils}/bin/install -m 600 /dev/null "$BOOTSTRAP_FILE" 2>/dev/null; then
      fail_over_to_slint "bootstrap file nao criavel (data-dir?)"
    fi
    printf '%s\n' "$BOOTSTRAP" > "$BOOTSTRAP_FILE"
    echo "jarbas-cinematic: launching Chromium JARBAS HUD"
    if run_cage ${pkgs.chromium}/bin/chromium \
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
      --user-data-dir="$XDG_RUNTIME_DIR/chromium-jarbas" \
      "''${LAB}#bootstrap=$BOOTSTRAP"; then
      rm -f "$BOOTSTRAP_FILE"
      exit 0
    fi
    rm -f "$BOOTSTRAP_FILE"
    fail_over_to_slint "renderer saiu"
  '';

  switchVt = pkgs.writeShellScript "jarbas-chvt" ''
    set -euo pipefail
    sleep 2
    ${pkgs.kbd}/bin/chvt "$(echo ${cfg.tty} | tr -dc '0-9')" || true
    MODE="${cfg.uiMode}"
    printf '\n*** JARBAS %s = %s � Alt+F%s ***\n' "$MODE" "${cfg.tty}" "$(echo ${cfg.tty} | tr -dc '0-9')" > /dev/tty1 || true
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
      type = lib.types.enum [ "auto" "cinematic" "slint" ];
      default = "slint";
      description = "auto selects Chromium only above the RAM budget and with its GPU profile validated; otherwise Slint.";
    };
    chromiumMinAvailableMiB = lib.mkOption {
      type = lib.types.ints.positive;
      default = 2048;
      description = "Minimum MemAvailable after nosd/AI startup required for Chromium in auto mode.";
    };
    chromiumGpuValidated = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Set true only after Chromium GPU acceleration is verified on this hardware profile.";
    };
  };

  config = lib.mkIf cfg.enable {
    services.seatd.enable = true;
    services.xserver.enable = lib.mkDefault false;

    # O lab é usado por Chromium no modo explícito ou auto (seleção runtime).
    systemd.services.nosd.serviceConfig.Environment = lib.mkAfter (
      lib.optional (builtins.elem cfg.uiMode [ "cinematic" "auto" ]) "NOSD_ORB_LAB=127.0.0.1:7701"
    );

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
      # StandardInput=tty em /dev/tty7 precisa de acesso ao grupo tty.
      "tty"
    ];

    fonts.packages = with pkgs; [ dejavu_fonts ];

    systemd.tmpfiles.rules = [
      "d /run/nosd/user 0700 nosd nosd -"
      "d /run/nosd/user/chromium-jarbas 0700 nosd nosd -"
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
      # QEMU/devtmpfs cria tty7 0600; cage (nosd) precisa abrir stdin.
      KERNEL=="tty7", GROUP="tty", MODE="0620"
    '';

    systemd.services.jarbas-hub = lib.mkIf cfg.hubFallback {
      description = "JARBAS hub slog (sem DRM)";
      path = [ pkgs.systemd pkgs.coreutils pkgs.util-linux pkgs.seatd ];
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
      # Regra 5: nada de comandos nus; waitScript usa mkdir/chown/systemctl.
      path = [ pkgs.systemd pkgs.coreutils pkgs.util-linux pkgs.seatd ];
      # Só arranca quando o GUI é pedido (hotswap pós-Q5 / persist-check).
      # Sem isto o cage arrancava numa VT inactiva durante o firstboot e
      # entrava em restart-loop (ecrã preto no tty7).
      unitConfig.ConditionPathExists = "/var/lib/nosd/gui-start";
      # Anti-loop: [Unit], nao [Service] (systemd ignora em serviceConfig).
      startLimitIntervalSec = 180;
      startLimitBurst = 3;
      wantedBy = lib.mkIf cfg.autoStart [ "multi-user.target" ];
      after = [
        "nosd.service"
        "nosd-early-score.service"
        "seatd.service"
        "jarbas-hub.service"
        "getty@tty1.service"
      ] ++ lib.optionals (builtins.elem cfg.uiMode [ "cinematic" "auto" ]) [
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
        # Anti-loop: Chromium reiniciava em spam (prompt+mouse sem UI est�vel)
        Restart = "on-failure";
        RestartSec = "10s";
        TimeoutStopSec = "20s";
        # seat = libseat/seatd; sem isto DRM master falha mesmo com DeviceAllow.
        SupplementaryGroups = "video audio render input seat tty";
        # Confinamento host thin (I.3): Preferir cage vivo a ProtectSystem=strict
        # (quebra seatd/DRM no metal � li��o PAMName/seat). Landlock syscall = nosd bin.
        ProtectHome = true;
        NoNewPrivileges = true;
        # DeviceAllow implica DevicePolicy=closed; tty7 (StandardInput=tty) tem
        # de estar na allowlist, senão systemd falha com 208/STDIN.
        # char-video4linux: webcam do kiosk Chromium (getUserMedia) sob DevicePolicy=closed.
        DeviceAllow = [ "char-drm rw" "char-input rw" "char-tty rw" "char-video4linux rw" ];
        OOMScoreAdjust = -500;
        # PATH: wrap usa paths absolutos; curl bare no PATH evita regress�o
        Environment = [
          "XDG_RUNTIME_DIR=/run/nosd/user"
          "NOSD_SOCKET=/run/nosd/nosd.sock"
          "RUST_LOG=info,jarbas=info"
          "PATH=${pkgs.systemd}/bin:${pkgs.curl}/bin:${pkgs.coreutils}/bin:${pkgs.cage}/bin:${pkgs.seatd}/bin"
        ];
      };
    };

    systemd.services.jarbas-show-vt = lib.mkIf cfg.autoStart {
      description = "JARBAS chvt -> UI tty";
      wantedBy = [ "multi-user.target" ];
      after = [ "jarbas-cage.service" ];
      wants = [ "jarbas-cage.service" ];
      # Não roubar a VT enquanto o firstboot pergunta no tty1: só salta para a
      # UI quando o questionário terminou (ou o sistema já está em aios-mode,
      # caso em que `nosd-boot-persist-check` escreve o marcador).
      unitConfig.ConditionPathExists = "/var/lib/nosd/firstboot.done";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "+${switchVt}";
        RemainAfterExit = true;
      };
    };
  };
}
