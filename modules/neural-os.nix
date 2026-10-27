# NixOS module — nosd systemd Type=notify (ADR-0118)
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
      # R-01/NX.3: journal legível + DRM.
      extraGroups = [ "video" "render" "systemd-journal" ];
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
      # Sobe apos local-fs (NAO esperar network-online; orb lab e opt-in cinematic)
      after = [ "local-fs.target" ];
      # R-01: serviços NixOS não herdam o PATH de login; declarar ferramentas.
      # (É atributo do serviço, NÃO de serviceConfig.)
      path = with pkgs; [
        networkmanager
        nix
        util-linux
        gnutar
        alsa-utils
        pipewire
        llama-cpp
        whisper-cpp
        espeak-ng
      ];
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
        # Prompt Minimal Image: proteger inferência do OOM killer (viável).
        OOMScoreAdjust = -1000;
        # RT suave só no unit nosd — não PAM global rtprio=99 (perigoso).
        LimitRTPRIO = 50;
        LimitMEMLOCK = "infinity";
        CPUWeight = 800;
        MemoryMax = "16G";
        RuntimeDirectory = "nosd";
        StateDirectory = "nosd";
        DeviceAllow = [ "char-drm rw" ];
        RestrictAddressFamilies = [ "AF_UNIX" "AF_INET" "AF_INET6" ];
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        # 0.5.x: Landlock activo (NOSD_LANDLOCK=1). FAT mount só se nixaiosLogFat.enable.
        ReadWritePaths =
          [ cfg.dataDir "/run/nosd" "/tmp" ]
          ++ lib.optionals (config.services.nixaiosLogFat.enable or false) [ "/mnt/nixaios-log" ];
        Environment = [
          "NOSD_ENGINE_DIR=${cfg.package}/lib/nosd/engines"
          "CUDA_VISIBLE_DEVICES=${cfg.cudaVisibleDevices}"
          "NOSD_HERMES_TICK_SECS=${toString cfg.hermesTickSecs}"
          "NOSD_MESH_BIND=${cfg.meshBind}"
          "NOSD_MESH_BROADCAST=${cfg.meshBroadcast}"
          # S-05: orb lab OFF por default; jarbas-cage liga só no modo cinematic.
          # I.3: Landlock no Linux (Absent/Failed honesto se kernel sem ABI)
          "NOSD_LANDLOCK=1"
          # AutoLearn/Seek fora do tick default (0.5.2); lab: NOSD_AUTOLEARN=1
          "NOSD_AUTOLEARN=0"
          # S1.2: early/ISO keyword; AgentJev só via LOO + NOSD_S1_BLOB
          "NOSD_S1_BACKEND=keyword"
          "RUST_LOG=info"
        ];
      };
    };
  };
}
