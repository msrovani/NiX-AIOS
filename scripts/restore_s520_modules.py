#!/usr/bin/env python3
"""Restore iso/neural-os/jarbas-cage/log-fat32 wiped by git checkout; LF only."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def w(rel: str, text: str) -> None:
    p = ROOT / rel
    data = text.replace("\r\n", "\n").replace("\r", "\n").encode("utf-8")
    p.write_bytes(data)
    assert b"\r" not in p.read_bytes()
    print(f"wrote {rel} ({len(data)} bytes)")


ISO = r'''# Offline ISO — GNOME Initial Setup -> nos-transmute -> Hotswap JARBAS (s520).
# Build: nix build .#iso && scripts/pack-usb-logfat.sh result/iso/*.iso ~/Downloads/nixaios-gnome-s520-….img
{ pkgs, lib, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/installer/cd-dvd/installation-cd-minimal.nix"
  ];

  isoImage.makeEfiBootable = true;
  isoImage.makeUsbBootable = true;
  isoImage.volumeID = "NIXAIOS520";

  services.nixaiosLogFat.enable = true;
  services.nixaiosTransmute.enable = true;

  services.nosdNixRing = {
    enable = true;
    autoSwitch = true;
  };

  networking.hostName = "nixaios";
  networking.networkmanager.enable = true;
  services.openssh.enable = lib.mkForce false;

  hardware.graphics.enable = true;
  hardware.graphics.enable32Bit = lib.mkDefault false;
  services.xserver.videoDrivers = lib.mkDefault [ "modesetting" ];

  services.getty.autologinUser = lib.mkForce null;

  services.nosd = {
    enable = true;
    dataDir = "/var/lib/nosd";
    hermesTickSecs = 2;
  };

  services.jarbasCage = {
    enable = true;
    nosdPackage = pkgs.nosd;
    drmDevice = "";
    tty = "tty7";
    autoStart = false;
    hubFallback = true;
    uiMode = "cinematic";
  };

  services.nosdAiManaged = {
    enable = true;
  };

  system.activationScripts.nosdModels = ''
    mkdir -p /var/lib/nosd/models
    mkdir -p /var/lib/nosd/nsgdb
    mkdir -p /run/user/nosd
    chown nosd:nosd /run/user/nosd || true
    chmod 700 /run/user/nosd || true
  '';

  environment.systemPackages = with pkgs; [
    pciutils
    vulkan-tools
    vulkan-loader
    mesa
    cage
    nosd
    chromium
    v4l-utils
    dejavu_fonts
    fontconfig
  ];

  boot.kernelModules = [ "uvcvideo" ];
  nix.settings.substituters = lib.mkDefault [ ];
  systemd.defaultUnit = lib.mkOverride 900 "graphical.target";
}
'''

NEURAL = r'''# NixOS module — nosd systemd Type=notify (ADR-0118)
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nosd;
in
{
  options.services.nosd = {
    enable = lib.mkEnableOption "Neural-OS Daemon (nosd)";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.nosd or pkgs.hello;
      description = "nosd package (flake should set pkgs.nosd)";
    };
    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/nosd";
    };
    socket = lib.mkOption {
      type = lib.types.str;
      default = "/run/nosd/nosd.sock";
    };
    nodeId = lib.mkOption {
      type = lib.types.ints.u32;
      default = 15;
    };
    cudaVisibleDevices = lib.mkOption {
      type = lib.types.str;
      default = "0";
      description = "CUDA_VISIBLE_DEVICES for dGPU compute";
    };
    hermesTickSecs = lib.mkOption {
      type = lib.types.ints.u32;
      default = 2;
      description = "NOSD_HERMES_TICK_SECS — loop Hermes apos boot";
    };
    meshBind = lib.mkOption {
      type = lib.types.str;
      default = "0.0.0.0:19000";
      description = "NOSD_MESH_BIND — UDP mesh HB; vazio = off";
    };
    meshBroadcast = lib.mkOption {
      type = lib.types.str;
      default = "255.255.255.255:19000";
      description = "NOSD_MESH_BROADCAST";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.nosd = {
      isSystemUser = true;
      group = "nosd";
      extraGroups = [ "video" "render" ];
    };
    users.groups.nosd = { };

    systemd.tmpfiles.rules = [
      "d ${cfg.dataDir} 0750 nosd nosd -"
      "d ${cfg.dataDir}/nsgdb 0750 nosd nosd -"
      "d ${cfg.dataDir}/models 0755 nosd nosd -"
      "d /run/nosd 0755 nosd nosd -"
    ];

    boot.kernel.sysctl."vm.nr_hugepages" = lib.mkDefault 256;

    systemd.services.nosd = {
      description = "Neural-OS Daemon";
      wantedBy = [ "multi-user.target" "graphical.target" ];
      # Sobe apos local-fs (NAO esperar network-online — senao :7701 recusa antes do Wi-Fi)
      after = [ "local-fs.target" ];
      serviceConfig = {
        Type = "notify";
        NotifyAccess = "main";
        User = "nosd";
        Group = "nosd";
        ExecStart = "${cfg.package}/bin/nosd --foreground --socket ${cfg.socket} --data-dir ${cfg.dataDir} --node-id ${toString cfg.nodeId} --hermes-tick-secs ${toString cfg.hermesTickSecs} --mesh-bind ${cfg.meshBind} --mesh-broadcast ${cfg.meshBroadcast}";
        Restart = "always";
        RestartSec = "2s";
        TimeoutStartSec = "60s";
        WatchdogSec = "0";
        Nice = -10;
        CPUWeight = 800;
        MemoryMax = "16G";
        RuntimeDirectory = "nosd";
        StateDirectory = "nosd";
        DeviceAllow = [ "char-drm rw" ];
        RestrictAddressFamilies = [ "AF_UNIX" "AF_INET" "AF_INET6" ];
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ReadWritePaths = [ cfg.dataDir "/run/nosd" "/mnt/nixaios-log" "/tmp" ];
        Environment = [
          "NOSD_ENGINE_DIR=${cfg.package}/lib/nosd/engines"
          "CUDA_VISIBLE_DEVICES=${cfg.cudaVisibleDevices}"
          "NOSD_HERMES_TICK_SECS=${toString cfg.hermesTickSecs}"
          "NOSD_MESH_BIND=${cfg.meshBind}"
          "NOSD_MESH_BROADCAST=${cfg.meshBroadcast}"
          "NOSD_ORB_LAB=127.0.0.1:7701"
          "RUST_LOG=info"
        ];
      };
    };
  };
}
'''

LOGFAT = r'''# Particao FAT32 LABEL=NIXAIOSLOG -> /mnt/nixaios-log (logs texto, legivel no Windows).
# journald binario fica em RAM; exportamos texto para a FAT (FAT != journald persistent).
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nixaiosLogFat;
  mnt = cfg.mountPoint;
in
{
  options.services.nixaiosLogFat = {
    enable = lib.mkEnableOption "NIXAIOSLOG FAT32 log partition";
    label = lib.mkOption {
      type = lib.types.str;
      default = "NIXAIOSLOG";
    };
    mountPoint = lib.mkOption {
      type = lib.types.str;
      default = "/mnt/nixaios-log";
    };
  };

  config = lib.mkIf cfg.enable {
    boot.supportedFilesystems = [ "vfat" ];

    fileSystems.${mnt} = {
      device = "/dev/disk/by-label/${cfg.label}";
      fsType = "vfat";
      options = [
        "rw"
        "nofail"
        "noatime"
        "umask=000"
        "gid=0"
        "uid=0"
        "x-systemd.device-timeout=12s"
        "x-systemd.after=local-fs-pre.target"
      ];
    };

    systemd.tmpfiles.rules = [
      "d ${mnt} 0755 root root -"
    ];

    systemd.services.nixaios-log-export = {
      description = "Export journal -> NIXAIOSLOG FAT32";
      wantedBy = [ "multi-user.target" ];
      after = [ "local-fs.target" "nosd.service" ];
      wants = [ "local-fs.target" ];
      serviceConfig = {
        Type = "simple";
        Restart = "on-failure";
        RestartSec = "10s";
        ExecStart = pkgs.writeShellScript "nixaios-log-export" ''
          set -euo pipefail
          MNT="${mnt}"
          LABEL="${cfg.label}"
          for i in $(seq 1 30); do
            if mountpoint -q "$MNT" 2>/dev/null; then break; fi
            if [ -e "/dev/disk/by-label/$LABEL" ]; then
              mkdir -p "$MNT"
              mount -t vfat -o rw,umask=000 "/dev/disk/by-label/$LABEL" "$MNT" 2>/dev/null || true
            fi
            sleep 1
          done
          if ! mountpoint -q "$MNT" 2>/dev/null; then
            echo "nixaios-log-export: $LABEL nao montado — so RAM journal" >&2
            exit 0
          fi
          STAMP=$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || echo boot)
          {
            echo "=== nixaios journal dump $STAMP ==="
            ${pkgs.systemd}/bin/journalctl -b -u nosd -u jarbas-cage -u jarbas-hub -u jarbas-show-vt -u nosd-hotswap-aios -u gdm-unstamp-initial-setup -u nixaios-log-export --no-pager || true
          } > "$MNT/journal-boot.log"
          sync
          exec ${pkgs.systemd}/bin/journalctl -f -u nosd -u jarbas-cage -u jarbas-hub -u nosd-hotswap-aios -o short-iso \
            >> "$MNT/journal-follow.log"
        '';
      };
    };

    environment.variables.NIXAIOS_LOG_DIR = mnt;
  };
}
'''

# jarbas-cage: full cinematic module from s515+s520 (conflicts display-manager)
JARBAS = r'''# JARBAS Wayland kiosk — ADR-0118
# s515: UI cinematica = Chromium kiosk -> orb lab; iGPU preferida.
# s520: conflicts display-manager (hotswap DRM).
{ config, lib, pkgs, ... }:
let
  cfg = config.services.jarbasCage;

  waitScript = pkgs.writeShellScript "jarbas-wait-deps" ''
    set -euo pipefail
    for i in $(seq 1 60); do
      if [ -S /run/nosd/nosd.sock ]; then break; fi
      sleep 0.5
    done
    for i in $(seq 1 40); do
      if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then break; fi
      sleep 0.5
    done
    mkdir -p /run/user/nosd
    chown nosd:nosd /run/user/nosd || true
    chmod 700 /run/user/nosd || true
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
    if mountpoint -q /mnt/nixaios-log 2>/dev/null && [ -w /mnt/nixaios-log ]; then
      LOG=/mnt/nixaios-log/jarbas-cage.log
    else
      LOG=/run/jarbas-cage.log
    fi
    exec > >(tee -a "$LOG") 2>&1
    echo "=== jarbas-cinematic start $(date -Is 2>/dev/null || echo now) uid=$(id -u) log=$LOG ==="
    export XDG_RUNTIME_DIR=/run/user/nosd
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=cage
    export NOSD_SOCKET=/run/nosd/nosd.sock
    export RUST_LOG="''${RUST_LOG:-info}"
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
    curl -sf -o /dev/null http://127.0.0.1:7701/ && echo "orb_lab: OK" || echo "orb_lab: DOWN"

    LAB="http://127.0.0.1:7701/"

    if [ "${cfg.uiMode}" = "slint" ]; then
      export FONTCONFIG_FILE="${pkgs.fontconfig.out}/etc/fonts/fonts.conf"
      export SLINT_DEFAULT_FONT="${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf"
      if [ "''${JARBAS_FORCE_SOFTWARE:-0}" = "1" ]; then
        export SLINT_BACKEND=winit-software
        export LIBGL_ALWAYS_SOFTWARE=1
      else
        unset SLINT_BACKEND || true
        unset LIBGL_ALWAYS_SOFTWARE || true
      fi
      exec ${pkgs.cage}/bin/cage -s -- \
        ${cfg.nosdPackage}/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock --interval-secs 2
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
    sleep 5
    ${pkgs.kbd}/bin/chvt "$(echo ${cfg.tty} | tr -dc '0-9')" || true
    printf '\n*** JARBAS (webcam UI) = %s — Alt+F%s ***\n' "${cfg.tty}" "$(echo ${cfg.tty} | tr -dc '0-9')" > /dev/tty1 || true
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
      default = "cinematic";
      description = "cinematic = Chromium orb lab; slint = HUD nativo.";
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
      description = "JARBAS cinematic kiosk (webcam orb lab / iGPU)";
      wantedBy = lib.mkIf cfg.autoStart [ "multi-user.target" ];
      after = [
        "nosd.service"
        "seatd.service"
        "pipewire.service"
        "wireplumber.service"
        "jarbas-hub.service"
        "getty@tty1.service"
        "mnt-nixaios\\x2dlog.mount"
      ];
      wants = [ "nosd.service" "seatd.service" "pipewire.service" "mnt-nixaios\\x2dlog.mount" ];
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
        Restart = "on-failure";
        RestartSec = "5s";
        StartLimitIntervalSec = 120;
        StartLimitBurst = 8;
        TimeoutStopSec = "20s";
        SupplementaryGroups = "video audio render input";
        Environment = [
          "XDG_RUNTIME_DIR=/run/user/nosd"
          "NOSD_SOCKET=/run/nosd/nosd.sock"
          "RUST_LOG=info"
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
'''


def main() -> None:
    w("images/iso.nix", ISO)
    w("modules/neural-os.nix", NEURAL)
    w("modules/log-fat32.nix", LOGFAT)
    w("modules/jarbas-cage.nix", JARBAS)


if __name__ == "__main__":
    main()
