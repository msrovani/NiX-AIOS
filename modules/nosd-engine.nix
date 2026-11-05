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
    # Credencial (LoadCredential=), não Environment=: a key nunca entra na
    # store, no `systemctl show` nem no argv do processo (--api-key-file).
    KEYFILE=""
    if [ -n "''${CREDENTIALS_DIRECTORY:-}" ] && [ -f "$CREDENTIALS_DIRECTORY/llama-api-key" ]; then
      KEYFILE="--api-key-file=$CREDENTIALS_DIRECTORY/llama-api-key"
    fi
    # Paridade com o spawn do nosd (nos-cortex): sem `-c` fixo, `--fit on`
    # escolhe o maior contexto/NGL que cabe na VRAM; `--reasoning off` evita
    # o orçamento gasto em raciocínio do Qwen3.x. Estava SÓ no spawn, o que
    # tornava `supervise=true` um motor diferente do resto do sistema.
    # `-ngl` fixo ANULA o `--fit`: com 99 um 4B Q6 rebenta em 6 GiB
    # (`cudaMalloc` OOM, metal 2026-11-03). Só passa quando configurado.
    exec ${cfg.llamaPackage}/bin/llama-server \
      --host 127.0.0.1 \
      --port "$PORT" \
      -m "$MODEL" \
      --fit on \
      --fit-ctx ${toString cfg.fitCtx} \
      --reasoning ${cfg.reasoning} \
      --no-webui \
      ${lib.optionalString (cfg.nGpuLayers > 0) "-ngl ${toString cfg.nGpuLayers}"} \
      ''${KEYFILE:-}
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
      default = 0;
      description = ''
        Camadas a offloadar. **0 = deixa `--fit on` decidir** (máximo que cabe
        na VRAM). Valor fixo alto anula o fit: `-ngl 99` num 4B Q6 com 6 GiB já
        parcialmente ocupados mata o arranque com `cudaMalloc: out of memory`
        (metal 2026-11-03, `nosd-engine-s2.service: Failed with result
        'exit-code'`; só recuperou num restart com NGL 5).
      '';
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
    fitCtx = lib.mkOption {
      type = lib.types.ints.positive;
      default = 2048;
      description = ''
        `--fit-ctx` mínimo: sem `-c` fixo, o llama.cpp escolhe o maior contexto
        que cabe na VRAM, nunca abaixo deste valor.
      '';
    };
    reasoning = lib.mkOption {
      type = lib.types.enum [ "on" "off" "auto" ];
      default = "off";
      description = "`--reasoning`: off evita que o Qwen3.x gaste o orçamento em  thinking";
    };
    apiKeyFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Ficheiro com a(s) API key(s) do llama-server (uma por linha). Entra no
        unit por `LoadCredential=` e é passado por `--api-key-file`, nunca por
        `Environment=`/argv: a store e o `systemctl show` são legíveis por
        todos, e o argv é visível em /proc. Verificado no pin atual:
        `llama-server --api-key-file FNAME` existe (llama-cpp 0.6.0).
      '';
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
          # DeviceAllow resolve aliases por /proc/devices (Linux-only, unit):
          # `char-nvidia` cobre /dev/nvidiaN + /dev/nvidiactl (major 195).
          # `char-nvidia-frontend` só existe com o módulo vGPU/Xen e
          # `char-nvidia-ctl` NÃO é alias (nvidiactl pertence ao major
          # `nvidia`): sob DevicePolicy=closed o systemd ignora-os em
          # silêncio e a CUDA morre a abrir /dev/nvidia0
          # (Permission denied → ENODEV do runtime CUDA). Ver
          # checks.hw-cuda-deviceallow no flake.
          DevicePolicy = "closed";
          DeviceAllow = [
            "char-nvidia rw"
            "char-nvidia-uvm rw"
            "char-nvidia-caps rw"
          ];
          SupplementaryGroups = [ "video" "render" ];
          LoadCredential = lib.optional (cfg.apiKeyFile != null) "llama-api-key:${cfg.apiKeyFile}";
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
          # Mesma regra dos units acima: `char-nvidia` cobre /dev/nvidiaN +
          # /dev/nvidiactl (major 195); frontend/ctl não são aliases válidos
          # em metal (ver checks.hw-cuda-deviceallow).
          DeviceAllow = [
            "char-nvidia rw"
            "char-nvidia-uvm rw"
            "char-nvidia-caps rw"
          ];
          SupplementaryGroups = [ "video" "render" ];
          LoadCredential = lib.optional (cfg.apiKeyFile != null) "llama-api-key:${cfg.apiKeyFile}";
          OOMScoreAdjust = 200;
          MemoryMax = "24G";
          MemoryHigh = "20G";
          TasksMax = 512;
          ExecStart = ''
            ${cfg.llamaPackage}/bin/llama-server \
              --host 127.0.0.1 \
              --port ${toString cfg.port} \
              -m ${cfg.modelPath} \
              --fit on \
              --fit-ctx ${toString cfg.fitCtx} \
              --reasoning ${cfg.reasoning} \
              --no-webui \
              ${lib.optionalString (cfg.nGpuLayers > 0) "-ngl ${toString cfg.nGpuLayers}"} \
              ${lib.optionalString (cfg.apiKeyFile != null) "--api-key-file=%d/llama-api-key"} \
              ${lib.concatStringsSep " " cfg.extraArgs}
          '';
        };
      };

      # T2.2: o nosd anexa ao motor por HTTP (backend externo já existente,
      # `NOSD_LLAMA_SERVER`) — deixa de spawnar filho próprio.
      systemd.services.nosd = {
        after = [ "nosd-engine.service" ];
        wants = [ "nosd-engine.service" ];
        serviceConfig = lib.mkMerge [
          {
            Environment = [
              "NOSD_LLAMA_SERVER=http://127.0.0.1:${toString cfg.port}"
            ];
          }
          (lib.mkIf (cfg.apiKeyFile != null) {
            # G2/T2.3: com key no motor, o attach tem de autenticar. A key
            # entra pela MESMA credencial (o nos-cortex resolve por
            # `NOSD_LLAMA_SERVER_KEY_FILE`); pô-la em `Environment=` vazaria
            # em `systemctl show -p Environment` — o que esta revisão corrigiu.
            # `%d` (v247+) = $CREDENTIALS_DIRECTORY do unit.
            LoadCredential = [ "llama-api-key:${cfg.apiKeyFile}" ];
            Environment = [ "NOSD_LLAMA_SERVER_KEY_FILE=%d/llama-api-key" ];
          })
        ];
      };
    })
  ]);
}
