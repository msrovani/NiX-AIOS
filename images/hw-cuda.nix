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
  ];
  # O unit já tem DeviceAllow=char-drm (política closed implícita): sem estes
  # nós a CUDA falha com ENODEV, mesmo com o driver carregado.
  systemd.services.nosd.serviceConfig.DeviceAllow = lib.mkAfter [
    "char-nvidia-frontend rw"
    "char-nvidia-uvm rw"
    "char-nvidia-caps rw"
    "char-nvidia-ctl rw"
  ];

  # nvidia-smi no perfil: capacity/energia usam a allowlist absoluta.
  environment.systemPackages = [ config.hardware.nvidia.package ];
}
