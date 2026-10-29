# Estagio 0: firstboot TEXTO no tty1 (sem GNOME) ? nosctl transmute ? Hotswap JARBAS.
# 0.5.x: GNOME/GIS/Chromium autostart falhou no metal; entrega = questionário bash + cage.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nixaiosTransmute;

  # 0.5.2: hotswap script externo (modules/transmute/hotswap-aios.sh)
  # nixpkgs 26.11 removeu substituteAll; replaceVars + runCommand.
  hotswapRaw = pkgs.replaceVars ./transmute/hotswap-aios.sh {
    curl = pkgs.curl;
    kbd = pkgs.kbd;
    nosd = config.services.nosd.package;
  };
  hotswapScript = pkgs.runCommand "nosd-hotswap-aios" { } ''
    cp ${hotswapRaw} $out
    chmod +x $out
  '';

  revertGuiScript = pkgs.writeShellScript "nosd-revert-gui" ''
    set -euo pipefail
    rm -f /var/lib/nosd/aios-mode 2>/dev/null || true
    systemctl stop jarbas-cage.service 2>/dev/null || true
    echo "revert: jarbas-cage parado; firstboot pode correr de novo (apague firstboot.done)"
  '';

  # Questionário texto: Q1 rede ? Q2 idioma/voz ? Q3 teclado ? Q4 fuso ? Q5 confirmar.
  # 0.5.x: rede primeiro; resume; log SEMPRE em /tmp (user nixos nao escreve /var/lib/nosd).
  # 0.5.2: firstboot script externo (modules/transmute/firstboot.sh)
  firstbootRaw = pkgs.replaceVars ./transmute/firstboot.sh {
    utilLinux = pkgs.util-linux;
    networkmanager = pkgs.networkmanager;
    iproute2 = pkgs.iproute2;
    kbd = pkgs.kbd;
    curl = pkgs.curl;
    systemd = pkgs.systemd;
    nosd = config.services.nosd.package;
    python3 = pkgs.python3;
  };
  firstbootScript = pkgs.runCommand "nos-firstboot-pkg" { } ''
    mkdir -p $out/bin
    cp ${firstbootRaw} $out/bin/nos-firstboot
    chmod +x $out/bin/nos-firstboot
  '';

  startGuiScript = pkgs.writeShellScriptBin "nos-start-gui" ''
    set -euo pipefail
    SUDO=/run/wrappers/bin/sudo
    [ -x "$SUDO" ] || SUDO=sudo
    echo "nos-start-gui: reset + start jarbas-cage + chvt 7"
    $SUDO systemctl start nosd.service || true
    $SUDO systemctl reset-failed jarbas-cage.service 2>/dev/null || true
    $SUDO systemctl start nosd-hotswap-aios.service 2>/dev/null || true
    $SUDO systemctl start jarbas-cage.service
    sleep 2
    if systemctl is-active --quiet jarbas-cage.service; then
      $SUDO ${pkgs.kbd}/bin/chvt 7
      echo "OK: cage active — Alt+F7"
      systemctl is-active jarbas-cage.service
    else
      echo "FAIL: cage still down"
      systemctl status jarbas-cage.service --no-pager -l || true
      journalctl -u jarbas-cage -b --no-pager -n 30 || true
      ls -la /dev/dri || true
      exit 1
    fi
  '';

  transmuteCli = pkgs.writeShellScriptBin "nos-transmute" ''
    set -euo pipefail
    CMD="''${1:-run}"
    SUDO=/run/wrappers/bin/sudo
    [ -x "$SUDO" ] || SUDO=sudo
    case "$CMD" in
      gui|firstboot)
        exec ${firstbootScript}/bin/nos-firstboot
        ;;
      status)
        exec ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute-status
        ;;
      run|auto)
        shift || true
        # S-01: transmute exige NIX_WRITE; user nixos não é admin do daemon.
        exec $SUDO ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute "$@"
        ;;
      hotswap)
        exec $SUDO ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute --no-download
        ;;
      revert-gui)
        exec sudo ${revertGuiScript}
        ;;
      *)
        echo "Uso: nos-transmute [firstboot | status | run | hotswap | revert-gui]"
        exit 1
        ;;
    esac
  '';

  # Wrapper com passthru.shellPath (NixOS exige "shell package").
  aiosShellScript = pkgs.writeShellScript "nixos-aios-shell" ''
    ${lib.optionalString cfg.firstbootAuto "export NOSD_FIRSTBOOT_AUTO=1"}
    {
      echo ""
      echo "**********************************************"
      echo "* Neural AIOS — shell de bootstrap           *"
      echo "**********************************************"
    } > /dev/tty 2>/dev/null || true

    TTY_NOW=$(tty 2>/dev/null || echo unknown)
    echo "[aios] tty=$TTY_NOW" > /dev/tty 2>/dev/null || true

    need_fb=1
    if [ -f /var/lib/nosd/firstboot.done ]; then
      need_fb=0
    fi
    if [ -f /var/lib/nosd/aios-mode ]; then
      mode=$(tr -d ' \n\r' < /var/lib/nosd/aios-mode 2>/dev/null || true)
      if [ "$mode" = "aios" ]; then need_fb=0; fi
    fi

    case "$TTY_NOW" in
      /dev/tty[0-9]*|/dev/ttyS[0-9]*|/dev/hvc[0-9]*|/dev/console)
        if [ "$need_fb" = "1" ] && [ -z "''${NOS_FIRSTBOOT_RAN:-}" ]; then
          export NOS_FIRSTBOOT_RAN=1
          echo "[aios] a iniciar nos-firstboot…" > /dev/tty 2>/dev/null || true
          ${firstbootScript}/bin/nos-firstboot || echo "[aios] firstboot exit=$?" > /dev/tty 2>/dev/null || true
        fi
        ;;
    esac

    export NOS_FIRSTBOOT_RAN=1
    exec ${pkgs.bashInteractive}/bin/bash -l "$@"
  '';

  loginShell = pkgs.runCommand "nixos-aios-shell" {
    passthru = { shellPath = "/bin/nixos-aios-shell"; };
  } ''
    mkdir -p $out/bin
    cp ${aiosShellScript} $out/bin/nixos-aios-shell
    chmod 755 $out/bin/nixos-aios-shell
  '';
in
{
  options.services.nixaiosTransmute = {
    enable = lib.mkEnableOption "Firstboot texto tty1 + transmute + Hotswap JARBAS";
    firstbootAuto = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Firstboot sem prompts (ISO/lab/headless): usa defaults, espera rede
        8s e corre `transmute --no-download`. Num install real deixar false.
        Override em runtime: NOSD_FIRSTBOOT_ASK=1 (forca interactivo) /
        NOSD_FIRSTBOOT_DOWNLOAD=1 (permite download em auto).
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    time.timeZone = lib.mkDefault "America/Sao_Paulo";
    i18n.defaultLocale = lib.mkDefault "pt_BR.UTF-8";
    i18n.supportedLocales = lib.mkDefault [
      "pt_BR.UTF-8/UTF-8"
      "pt_PT.UTF-8/UTF-8"
      "en_US.UTF-8/UTF-8"
      "es_ES.UTF-8/UTF-8"
    ];
    console.keyMap = lib.mkDefault "br";

    # SEM GNOME / GDM — console only ate hotswap.
    services.xserver.enable = lib.mkForce false;
    services.displayManager.gdm.enable = lib.mkForce false;
    services.desktopManager.gnome.enable = lib.mkForce false;
    services.xserver.displayManager.gdm.enable = lib.mkForce false;
    services.xserver.desktopManager.gnome.enable = lib.mkForce false;
    services.displayManager.autoLogin.enable = lib.mkForce false;

    services.getty.autologinUser = lib.mkForce "nixos";

    # Shell wrapper = disparo fiavel (nao depende de profile.d)
    users.users.nixos.shell = lib.mkForce loginShell;

    # Backup 1: loginShellInit
    environment.loginShellInit = lib.mkAfter ''
      ${lib.optionalString cfg.firstbootAuto "export NOSD_FIRSTBOOT_AUTO=1"}
      if [ -z "''${NOS_FIRSTBOOT_RAN:-}" ]; then
        case "$(tty 2>/dev/null)" in
          /dev/tty[0-9]*|/dev/ttyS[0-9]*|/dev/hvc[0-9]*|/dev/console)
            if [ ! -f /var/lib/nosd/firstboot.done ]; then
              export NOS_FIRSTBOOT_RAN=1
              ${firstbootScript}/bin/nos-firstboot || true
            fi
            ;;
        esac
      fi
    '';

    # Backup 2: .bash_profile no home (sobrevive se shell voltar a bash)
    system.activationScripts.nosFirstbootBashProfile = {
      text = ''
        mkdir -p /home/nixos
        cat > /home/nixos/.bash_profile << 'EOF'
${lib.optionalString cfg.firstbootAuto "export NOSD_FIRSTBOOT_AUTO=1"}
# Neural AIOS firstboot (s524) — forced
echo "[aios] .bash_profile tty=$(tty 2>/dev/null)"
if [ -z "''${NOS_FIRSTBOOT_RAN:-}" ]; then
  case "$(tty 2>/dev/null)" in
    /dev/tty*|/dev/ttyS*|/dev/hvc*|/dev/console)
      if [ ! -f /var/lib/nosd/firstboot.done ]; then
        export NOS_FIRSTBOOT_RAN=1
        nos-firstboot || true
      fi
      ;;
  esac
fi
[ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"
EOF
        chown nixos:users /home/nixos/.bash_profile 2>/dev/null || true
        chmod 644 /home/nixos/.bash_profile 2>/dev/null || true
      '';
      deps = [ "users" ];
    };

    environment.etc.motd.text = ''

      Neural AIOS — questionario: nos-firstboot
      GUI (tty7):            nos-start-gui
      Se ja fez Q1–Q5:       sudo systemctl start jarbas-cage && sudo chvt 7

    '';

    security.sudo.enable = true;
    security.sudo.wheelNeedsPassword = false;
    # Garante wrapper setuid em /run/wrappers/bin/sudo (nao usar pkgs.sudo/bin)
    services.jarbasCage.autoStart = lib.mkForce false;

    environment.systemPackages = with pkgs; [
      firstbootScript
      transmuteCli
      startGuiScript
      loginShell
      networkmanager
      curl
      kbd
      llama-cpp
      whisper-cpp
      espeak-ng
      alsa-utils
    ];

    systemd.paths.nosd-hotswap-aios = {
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = "/var/lib/nosd/hotswap-aios.request";
        Unit = "nosd-hotswap-aios.service";
      };
    };

    systemd.services.nosd-hotswap-aios = {
      description = "Hotswap texto/bootstrap ? JARBAS AIOS Kiosk";
      after = [ "nosd.service" ];
      # R-01: o script usa tee/systemctl/journalctl/mv/cat (nus) - sem path
      # o unit morria logo no `exec > >(tee ...)`. Nao usar serviceConfig.path.
      path = [ pkgs.bash pkgs.systemd pkgs.coreutils pkgs.util-linux pkgs.kbd pkgs.curl ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${hotswapScript}";
      };
    };

    # 0.5.x: sem FAT — resume via /var/lib/nosd/aios-mode (overlay Linux)
    systemd.services.nosd-boot-persist-check = {
      description = "Auto-enter JARBAS if /var/lib/nosd already transmuted";
      wantedBy = [ "multi-user.target" ];
      after = [ "nosd.service" "getty@tty1.service" ];
      path = [ pkgs.systemd pkgs.coreutils ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "nosd-boot-persist-check" ''
          sleep 3
          if [ -f /var/lib/nosd/aios-mode ]; then
            mode=$(tr -d ' \n\r' < /var/lib/nosd/aios-mode || true)
            if [ "$mode" = "aios" ]; then
              touch /var/lib/nosd/firstboot.done
              touch /var/lib/nosd/hotswap-aios.request
              systemctl start --no-block nosd-hotswap-aios.service || true
            fi
          fi
        '';
      };
    };
  };
}
