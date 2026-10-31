# Imagem HW real — **um único** GGUF: Qwen3.5-4B-Q6_K (~3.5 GB).
#
# Sem 0.8B/Q8/4B-Q4/9B na ISO (mais leve que hw.nix). CUDA em
# `images/hw-cuda-4b.nix`. NUNCA release.
{ lib, pkgs, self, ... }:
{
  services.nixaiosLogFat.enable = lib.mkForce true;
  services.nixaiosLogFat.seedModels = true;
  services.nixaiosLogFat.seedWifi = true;
  services.nixaiosTransmute.firstbootAuto = lib.mkForce false;
  services.jarbasCage.uiMode = lib.mkForce "cinematic";
  boot.kernelParams = lib.mkAfter [ "console=tty0" ];

  # Um modelo: nome canónico + `active.gguf` (slot Active; evita slot_map
  # mapear "4b" → GeneratorPro e deixar Active vazio).
  systemd.tmpfiles.rules = lib.mkAfter [
    "L+ /var/lib/nosd/models/Qwen3.5-4B-Q6_K.gguf - - - - ${self.packages.${pkgs.system}.qwen35-4b-q6k}"
    "L+ /var/lib/nosd/models/active.gguf - - - - ${self.packages.${pkgs.system}.qwen35-4b-q6k}"
  ];

  environment.systemPackages = [ pkgs.llama-cpp ];
  systemd.services.nosd.path = lib.mkAfter [ pkgs.llama-cpp ];
  environment.variables.NOSD_FIRSTBOOT_DOWNLOAD = "1";

  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "NOSD_BOOT_MODEL_REPO=unsloth/Qwen3.5-4B-GGUF"
    "NOSD_BOOT_MODEL_FILE=Qwen3.5-4B-Q6_K.gguf"
    "NOSD_BOOT_MODEL_LOCAL=active.gguf"
    "NOSD_BOOT_MODEL_ONLY=1"
    "NOSD_LLAMA_FIT_CTX=2048"
  ];
}
