# images/hw-cuda.nix — variante do iso-hw com NVIDIA proprietário + CUDA.
#
# Porque existe: o iso-hw base usa o driver **nouveau** (open-source, KMS) —
# dá display mas **não** dá CUDA. Numa RTX 3050 (Ampere) a inferência só
# acelera com o driver proprietário. O cage/JARBAS continua na iGPU (o
# `pickDrm` prefere Intel); o dGPU fica para o llama-server (CUDA).
#
# S1 = 0.8B **em CPU** (NOSD_LLAMA_NGL=0) e S2/PRO = 4B **em dGPU** (unit
# `nosd-engine-s2`, 127.0.0.1:8082). Os dois modelos viajam na imagem
# (`images/hw.nix` já pina os 5 GGUFs); o router decide por pedido.
#
# NUNCA release: unfree + driver + CUDA; é bancada/hw.
{ config, lib, pkgs, self, ... }:
{
  hardware.nvidia = {
    modesetting.enable = true;
    # Ampere (GA107) com módulos proprietários: caminho CUDA estável.
    open = false;
    powerManagement.enable = false;
    nvidiaSettings = false;
  };
  services.xserver.videoDrivers = lib.mkForce [ "nvidia" ];
  # mkAfter: hw.nix também acrescenta `console=tty0` (não sobrescrever).
  boot.kernelParams = lib.mkAfter [ "nvidia-drm.modeset=1" ];

  # S1 (0.8B) fica no binário CPU do hw.nix (`path` mkAfter). S2 usa o build
  # CUDA explicitamente no ExecStart — o daemon não precisa de CUDA para o S1.
  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "CUDA_VISIBLE_DEVICES=0"
    "NVIDIA_VISIBLE_DEVICES=0"
    # S1: 0.8B em CPU (forçado). Sem `NOSD_LLAMA_CTX`: `--fit` escolhe o ctx
    # que cabe (mínimo NOSD_LLAMA_FIT_CTX=2048 do hw.nix).
    "NOSD_LLAMA_NGL=0"
    "NOSD_LLAMA_MODEL=/var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf"
    # S2/PRO: 4B Q6_K (tier alpha.2 para 6 GB) no llama-server 8082.
    "NOSD_LLAMA_SERVER_S2=http://127.0.0.1:8082"
    "NOSD_LLAMA_MODEL_S2=/var/lib/nosd/models/Qwen3.5-4B-Q6_K.gguf"
    # nvidia-smi vive no output `bin` (o `out` só tem nvidia-sleep.sh);
    # caminho absoluto para capacity/energia/DIAG (allowlist do nosd).
    "NOSD_NVIDIA_SMI=${config.hardware.nvidia.package.bin}/bin/nvidia-smi"
  ];
  # DeviceAllow=... implica política `closed` (o unit base já tem char-drm,
  # neural-os.nix). Os aliases resolvem por /proc/devices (Linux-only):
  # `char-nvidia` cobre /dev/nvidiaN + /dev/nvidiactl (major 195).
  # `char-nvidia-frontend` só existe com o módulo vGPU/Xen e
  # `char-nvidia-ctl` NÃO é alias — sob closed o systemd ignora-os em
  # silêncio e a CUDA morre a abrir /dev/nvidia0
  # (Permission denied → ENODEV do runtime CUDA). Ver
  # checks.hw-cuda-deviceallow no flake.
  systemd.services.nosd.serviceConfig.DeviceAllow = lib.mkAfter [
    "char-nvidia rw"
    "char-nvidia-uvm rw"
    "char-nvidia-caps rw"
  ];

  # nvidia-smi no perfil: `out` só tem nvidia-sleep.sh; o binário está em `bin`.
  environment.systemPackages = lib.mkAfter [
    config.hardware.nvidia.package
    config.hardware.nvidia.package.bin
  ];

  # S2/PRO: 4B no dGPU, porta 8082 (loopback). Sem `-c` nem `-ngl` fixos:
  # `--fit on` escolhe camadas+ctx que cabem na VRAM (e o DIAG mostra a VRAM
  # usada — é essa a prova de que o 4B está na GPU).
  systemd.services.nosd-engine-s2 = {
    description = "llama-server S2/PRO (Qwen 4B Q6_K, CUDA)";
    after = [ "local-fs.target" ];
    wantedBy = [ "multi-user.target" ];
    path = [ pkgs.coreutils ];
    startLimitIntervalSec = 120;
    startLimitBurst = 5;
    serviceConfig = {
      Type = "simple";
      User = "nosd";
      Group = "nosd";
      Restart = "on-failure";
      RestartSec = "5s";
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "/var/lib/nosd" ];
      DevicePolicy = "closed";
      # `char-nvidia` cobre /dev/nvidiaN + /dev/nvidiactl (major 195);
      # frontend/ctl não são aliases válidos em metal (ver comentário
      # acima e checks.hw-cuda-deviceallow).
      DeviceAllow = [
        "char-nvidia rw"
        "char-nvidia-uvm rw"
        "char-nvidia-caps rw"
      ];
      SupplementaryGroups = [ "video" "render" ];
      OOMScoreAdjust = 200;
      MemoryMax = "8G";
      Environment = [ "CUDA_VISIBLE_DEVICES=0" ];
      ExecStart = "${self.packages.${pkgs.system}.llama-cpp-cuda}/bin/llama-server --host 127.0.0.1 --port 8082 -m /var/lib/nosd/models/Qwen3.5-4B-Q6_K.gguf --fit on --fit-ctx 2048 --reasoning off --no-webui";
    };
  };
}
