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
    # V0 (plano vampiro): pesos do score declarados (accuracy,latency,cost,efficiency).
    scoreWeights = lib.mkOption {
      type = lib.types.str;
      default = "0.6,0.2,0.1,0.1";
      description = "NOSD_SCORE_WEIGHTS — 4 valores; normalizados no daemon";
    };
    # V2: preços do contrafactual local-vs-nuvem (sempre com ref_date).
    kwhPriceEur = lib.mkOption {
      type = lib.types.float;
      default = 0.25;
      description = "NOSD_KWH_PRICE_EUR — tarifa usada no custo local";
    };
    cloudUsdIn = lib.mkOption {
      type = lib.types.float;
      default = 0.60;
      description = "NOSD_CLOUD_USD_IN — USD/1M tokens de entrada (referência)";
    };
    cloudUsdOut = lib.mkOption {
      type = lib.types.float;
      default = 2.40;
      description = "NOSD_CLOUD_USD_OUT — USD/1M tokens de saída (referência)";
    };
    priceRefDate = lib.mkOption {
      type = lib.types.str;
      default = "2026-10-29";
      description = "NOSD_PRICE_REF_DATE — data dos preços de referência";
    };
    priceRefModel = lib.mkOption {
      type = lib.types.str;
      default = "cloud API genérica 4B-9B (referência, não benchmark)";
      description = "NOSD_PRICE_REF_MODEL — modelo/API do contrafactual";
    };
    tdpWatts = lib.mkOption {
      type = lib.types.nullOr lib.types.float;
      default = null;
      description = ''
        NOSD_TDP_W — estimativa de energia (W) quando não há RAPL/NVML.
        null = sem estimativa (source=absent, zero joules).
      '';
    };
    # D1: caminho absoluto do nvidia-smi (o PATH do unit não decide).
    nvidiaSmi = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        NOSD_NVIDIA_SMI — caminho absoluto. null = allowlist do daemon
        (/run/current-system/sw/bin/nvidia-smi, /usr/bin, ...).
        Ex.: "${config.hardware.nvidia.package}/bin/nvidia-smi".
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.nosd = {
      isSystemUser = true;
      group = "nosd";
      # R-01/NX.3: journal legível + DRM.
      # `audio`: /dev/snd/* é root:audio 0660 — sem este grupo o `arecord`
      # (mic) e o `aplay` (TTS) falham com EACCES no metal (voz muda).
      extraGroups = [ "video" "render" "systemd-journal" "audio" ];
    };
    users.groups.nosd = { };
    # Idempotente: garante o grupo mesmo se nenhum módulo de áudio o definir.
    users.groups.audio = { };

    systemd.tmpfiles.rules = [
      "d ${cfg.dataDir} 0750 nosd nosd -"
      "d ${cfg.dataDir}/nsgdb 0750 nosd nosd -"
      "d ${cfg.dataDir}/models 0755 nosd nosd -"
      "d /run/nosd 0755 nosd nosd -"
    ];

    boot.kernel.sysctl."vm.nr_hugepages" = lib.mkDefault 256;

    # Persona default do JARBAS. Sem isto o prompt é só "## User" e o modelo
    # responde genérico/em inglês (visto no metal). Lido por
    # `cognitive_bridge::build_chat_prompt` via NOSD_SOUL_PATH.
    environment.etc."nosd/SOUL.md".text = ''
      Você é JARBAS, a interface neural do NixAIOS.
      Fale em português do Brasil, curto e direto (2 a 4 frases).
      Tom calmo, técnico, sem floreados; trate o utilizador por "você".
      Nunca finja capacidades: se falta motor, modelo ou rede, diga-o.
      Se não souber, pergunte em vez de inventar.
    '';

    # V0: falha no eval, não no runtime (o daemon também valida).
    assertions = [
      {
        assertion = builtins.length (lib.splitString "," cfg.scoreWeights) == 4;
        message = "services.nosd.scoreWeights: 4 valores (accuracy,latency,cost,efficiency)";
      }
      {
        assertion = cfg.kwhPriceEur >= 0.0;
        message = "services.nosd.kwhPriceEur não pode ser negativo";
      }
    ];

    systemd.services.nosd = {
      description = "Neural-OS Daemon";
      wantedBy = [ "multi-user.target" "graphical.target" ];
      # T0.6: anti-crash-loop é atributo do UNIT, não de serviceConfig
      # (lição 2 do AGENTS.md — o systemd ignora StartLimit* em serviceConfig
      # em silêncio; o cage já tinha a lição, o daemon não).
      startLimitIntervalSec = 120;
      startLimitBurst = 5;
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
        # observe: journalctl (D1 inventário de spawns; sem isto o snapshot
        # de journal fica vazio em NixOS — o PATH do unit é fechado).
        systemd
        # voice: `timeout` é coreutils (gravação com limite) — faltava no path.
        coreutils
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
          # V0/V2 (plano vampiro): score declarado + custo com fonte datada.
          "NOSD_SCORE_WEIGHTS=${cfg.scoreWeights}"
          "NOSD_KWH_PRICE_EUR=${toString cfg.kwhPriceEur}"
          "NOSD_CLOUD_USD_IN=${toString cfg.cloudUsdIn}"
          "NOSD_CLOUD_USD_OUT=${toString cfg.cloudUsdOut}"
          "NOSD_PRICE_REF_DATE=${cfg.priceRefDate}"
          "NOSD_PRICE_REF_MODEL=${cfg.priceRefModel}"
          # Persona default (environment.etc acima).
          "NOSD_SOUL_PATH=/etc/nosd/SOUL.md"
          "RUST_LOG=info"
        ] ++ lib.optional (cfg.tdpWatts != null) "NOSD_TDP_W=${toString cfg.tdpWatts}"
          ++ lib.optional (cfg.nvidiaSmi != null) "NOSD_NVIDIA_SMI=${cfg.nvidiaSmi}";
      };
    };
  };
}
