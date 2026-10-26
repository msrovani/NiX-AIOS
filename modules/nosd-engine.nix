# dGPU llama-server template — separado do cage JARBAS (iGPU).
# jarbas-cage.nix → WLR_DRM_DEVICES Intel/card0; este unit → CUDA ou card1 DRM.
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
  };

  config = lib.mkIf cfg.enable {
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
  };
}
