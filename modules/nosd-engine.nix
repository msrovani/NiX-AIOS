# Motor de inferência como SERVIÇO (G2/T2): llama-server supervisionado pelo
# systemd, não subprocesso do nosd.
# - `nosd-engine@` (template, dGPU) mantém-se: instância manual por porta.
# - `services.nosdEngine.supervise = true` cria `nosd-engine.service`
#   (concreto, port/modelPath declarados) e liga o nosd via HTTP externo
#   (`NOSD_LLAMA_SERVER`), eliminando a classe de bugs PATH/exit-127 de
#   subprocessos. Default OFF (o caminho spawn managed mantém o comportamento
#   dos VM tests: unload → cpu-echo → reload).
# dGPU via CUDA ou card1 DRM; o cage fica com a iGPU (/dev/dri/card0).
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nosdEngine;
  startScript = pkgs.writeShellScript "nosd-engine-at" ''
    set -euo pipefail
    PORT="''${1:?instance = TCP port, e.g. 8081}"
    MODEL="''${NOSD_ENGINE_MODEL:-${cfg.modelsDir}/default.gguf}"
    if [ ! -f "$MODEL" ]; then
      echo "nosd-engine@$PORT: missing model $MODEL (set NOSD_ENGINE_MODEL)" >&2
      exit 1
    fi
    export CUDA_VISIBLE_DEVICES="''${CUDA_VISIBLE_DEVICES:-${cfg.cudaVisibleDevices}}"
    export NVIDIA_VISIBLE_DEVICES="''${NVIDIA_VISIBLE_DEVICES:-${cfg.cudaVisibleDevices}}"
    exec ${cfg.llamaPackage}/bin/llama-server \
      --host 127.0.0.1 \
      --port "$PORT" \
      -m "$MODEL" \
      -ngl ${toString cfg.nGpuLayers}
  '';
in
{
  options.services.nosdEngine = {
    enable = lib.mkEnableOption "nosd-engine@ template (dGPU llama-server)";
    llamaPackage = lib.mkOption {
      type = lib.types.package;
      default = pkgs.llama-cpp;
      description = "llama.cpp; flake .#llama-cpp-cuda for GPU layers";
    };
    modelsDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/nosd/models";
    };
    nGpuLayers = lib.mkOption {
      type = lib.types.int;
      default = 99;
    };
    cudaVisibleDevices = lib.mkOption {
      type = lib.types.str;
      default = "0";
      description = "NVIDIA_VISIBLE_DEVICES / CUDA_VISIBLE_DEVICES (dGPU)";
    };
    drmDevice = lib.mkOption {
      type = lib.types.str;
      default = "/dev/dri/card1";
      description = "Reference DRM node for dGPU; cage must not pin this device";
    };

    # G2/T2: motor como serviço supervisionado.
    supervise = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Cria `nosd-engine.service` (llama-server supervisionado pelo systemd)
        e aponta o nosd para `NOSD_LLAMA_SERVER=http://127.0.0.1:<port>`
        (modo attach do LlamaServerBackend — sem spawn de subprocesso).
      '';
    };
    port = lib.mkOption {
      type = lib.types.port;
      default = 8080;
      description = "Porta do llama-server supervisionado";
    };
    modelPath = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "GGUF a servir (obrigatório com supervise=true)";
    };
    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "Flags extra do llama-server (ex.: --api-key, --no-webui)";
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      systemd.services."nosd-engine@" = {
        description = "nosd llama-server (dGPU) port %i";
        after = [ "nosd.service" "local-fs.target" ];
        wants = [ "nosd.service" ];
        serviceConfig = {
          Type = "simple";
          User = "nosd";
          Group = "nosd";
          Restart = "on-failure";
          RestartSec = "5s";
          # W2: dGPU via CUDA — sem DRM card (cage fica com iGPU /dev/dri).
          # DeviceAllow vazio + DevicePolicy=closed bloqueia DRM; NVIDIA char nodes via SupplementaryGroups.
          DevicePolicy = "closed";
          DeviceAllow = [
            "char-nvidia-frontend rw"
            "char-nvidia-uvm rw"
            "char-nvidia-caps rw"
          ];
          SupplementaryGroups = [ "video" "render" ];
          # Isolamento vs cage/iGPU: OOM do motor não deve preferir matar JARBAS.
          OOMScoreAdjust = 200;
          MemoryMax = "24G";
          MemoryHigh = "20G";
          TasksMax = 512;
          Environment = [
            "NOSD_ENGINE_GPU=dgpu"
            "CUDA_VISIBLE_DEVICES=${cfg.cudaVisibleDevices}"
            "NVIDIA_VISIBLE_DEVICES=${cfg.cudaVisibleDevices}"
          ];
          ExecStart = "${startScript} %i";
        };
      };

      environment.systemPackages = [ cfg.llamaPackage ];

      assertions = [
        {
          assertion = !cfg.supervise || cfg.modelPath != null;
          message = "services.nosdEngine.supervise=true exige services.nosdEngine.modelPath (GGUF)";
        }
      ];
    }
    (lib.mkIf cfg.supervise {
      # T2.1: serviço concreto com todo o sandbox declarado no unit (Regra 5:
      # nada de comandos nus — ExecStart absoluto). StartLimit no UNIT
      # (lição 2 do AGENTS.md).
      systemd.services."nosd-engine" = {
        description = "llama-server supervisionado (motor como serviço, G2)";
        after = [ "local-fs.target" ];
        wantedBy = [ "multi-user.target" ];
        startLimitIntervalSec = 120;
        startLimitBurst = 5;
        path = with pkgs; [ coreutils ];
        serviceConfig = {
          Type = "simple";
          User = "nosd";
          Group = "nosd";
          Restart = "on-failure";
          RestartSec = "5s";
          NoNewPrivileges = true;
          ProtectSystem = "strict";
          ProtectHome = true;
          ReadWritePaths = [ "${toString cfg.modelsDir}" ];
          RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
          DevicePolicy = "closed";
          DeviceAllow = [
            "char-nvidia-frontend rw"
            "char-nvidia-uvm rw"
            "char-nvidia-caps rw"
          ];
          SupplementaryGroups = [ "video" "render" ];
          OOMScoreAdjust = 200;
          MemoryMax = "24G";
          MemoryHigh = "20G";
          TasksMax = 512;
          ExecStart = ''
            ${cfg.llamaPackage}/bin/llama-server \
              --host 127.0.0.1 \
              --port ${toString cfg.port} \
              -m ${cfg.modelPath} \
              -ngl ${toString cfg.nGpuLayers} \
              ${lib.concatStringsSep " " cfg.extraArgs}
          '';
        };
      };

      # T2.2: o nosd anexa ao motor por HTTP (backend externo já existente,
      # `NOSD_LLAMA_SERVER`) — deixa de spawnar filho próprio.
      systemd.services.nosd = {
        after = [ "nosd-engine.service" ];
        wants = [ "nosd-engine.service" ];
        serviceConfig.Environment = [
          "NOSD_LLAMA_SERVER=http://127.0.0.1:${toString cfg.port}"
        ];
      };
    })
  ]);
}
