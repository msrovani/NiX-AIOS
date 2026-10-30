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
    "NOSD_LLAMA_NGL=99"
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

  # Brain maior para a 3050: tier 3 pinado (4B Q4_K_M, 2.55 GiB) em vez do
  # 0.8B. Cabe nos 6 GB com KV 4k; o iso-hw normal continua com 0.8B (CPU).
  services.nosdBrain = {
    enable = true;
    package = lib.mkForce self.packages.${pkgs.system}.qwen35-4b-q4km;
    name = lib.mkForce "qwen3.5-4b-q4km.gguf";
  };
}
