# images/hw-cuda.nix — variante do iso-hw com NVIDIA proprietário + CUDA.
#
# Porque existe: o iso-hw usa o driver **nouveau** (open-source, KMS) — dá
# display mas **não** dá CUDA. Numa RTX 3050 (Ampere) a inferência só acelera
# com o driver proprietário. O cage/JARBAS continua na iGPU (o `pickDrm`
# prefere Intel); o dGPU fica para o llama-server (CUDA).
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
  boot.kernelParams = [ "nvidia-drm.modeset=1" ];

  # `mkBefore`: o `llama-server` do CUDA ganha ao CPU no PATH do unit nosd.
  systemd.services.nosd.path = lib.mkBefore [
    self.packages.${pkgs.system}.llama-cpp-cuda
  ];
  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "CUDA_VISIBLE_DEVICES=0"
    "NVIDIA_VISIBLE_DEVICES=0"
    # S1: 0.8B em CPU (forçado). S2 vai ao serviço 8082 (4B CUDA).
    "NOSD_LLAMA_NGL=0"
    "NOSD_LLAMA_CTX=4096"
    "NOSD_LLAMA_MODEL=/var/lib/nosd/models/qwen3.5-0.8b-q4km.gguf"
    "NOSD_LLAMA_SERVER_S2=http://127.0.0.1:8082"
    "NOSD_LLAMA_MODEL_S2=/var/lib/nosd/models/qwen3.5-4b-q4km.gguf"
    # nvidia-smi vive no output `bin` (o `out` só tem nvidia-sleep.sh);
    # caminho absoluto para capacity/energia (allowlist do nosd).
    "NOSD_NVIDIA_SMI=${config.hardware.nvidia.package.bin}/bin/nvidia-smi"
  ];
  # O unit já tem DeviceAllow=char-drm (política closed implícita): sem estes
  # nós a CUDA falha com ENODEV, mesmo com o driver carregado.
  systemd.services.nosd.serviceConfig.DeviceAllow = lib.mkAfter [
    "char-nvidia-frontend rw"
    "char-nvidia-uvm rw"
    "char-nvidia-caps rw"
    "char-nvidia-ctl rw"
  ];

  # nvidia-smi no perfil: `out` só tem nvidia-sleep.sh; o binário está em `bin`.
  environment.systemPackages = [
    config.hardware.nvidia.package
    config.hardware.nvidia.package.bin
  ];

  # Dois modelos na MESMA imagem:
  #  - 0.8B (S1) → `services.nosdBrain` (symlink do módulo comum);
  #  - 4B (S2/PRO) → symlink próprio, servido pela unit nosd-engine-s2 em CUDA.
  systemd.tmpfiles.rules = [
    "L+ /var/lib/nosd/models/qwen3.5-4b-q4km.gguf - - - - ${self.packages.${pkgs.system}.qwen35-4b-q4km}"
  ];
  services.nosdBrain = {
    enable = true;
    package = lib.mkForce self.packages.${pkgs.system}.qwen35-08b-q4km;
    name = lib.mkForce "qwen3.5-0.8b-q4km.gguf";
  };

  # S2/PRO: 4B no dGPU, porta 8082 (loopback). S1 fica no nosd (CPU).
  systemd.services.nosd-engine-s2 = {
    description = "llama-server S2/PRO (Qwen 4B, CUDA)";
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
      DeviceAllow = [
        "char-nvidia-frontend rw"
        "char-nvidia-uvm rw"
        "char-nvidia-caps rw"
        "char-nvidia-ctl rw"
      ];
      SupplementaryGroups = [ "video" "render" ];
      OOMScoreAdjust = 200;
      MemoryMax = "8G";
      Environment = [ "CUDA_VISIBLE_DEVICES=0" ];
      ExecStart = "${self.packages.${pkgs.system}.llama-cpp-cuda}/bin/llama-server --host 127.0.0.1 --port 8082 -m /var/lib/nosd/models/qwen3.5-4b-q4km.gguf -ngl 99 -c 4096 --no-webui";
    };
  };
}
