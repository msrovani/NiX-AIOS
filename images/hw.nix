# Imagem para teste em HARDWARE real (nao e a ISO de release).
#
# Liga o export de logs para uma particao FAT32 com LABEL=NIXAIOSLOG:
#  - journal-boot.log / journal-follow.log (nosd, jarbas-*, hotswap)
#  - firstboot.log / hotswap.log / jarbas-cage.log (copias periodicas)
#  - dmesg-boot.log e hw-info.log (lspci/lsusb/uname) para analise pos-teste
# Nao abre SSH nem firewall; comportamento de rede igual a ISO de release.
#
# Modelo de arranque: Qwen3.5-0.8B (GGUF Q4_K_M, Apache-2.0) baixado no first
# boot via transmute (HITL) para /var/lib/nosd/models. O repo canonico
# `Qwen/Qwen3.5-0.8B` e safetensors (VLM) e nao serve ao llama.cpp; usamos o
# GGUF do unsloth. Texto apenas (vision/mmproj fica para depois).
{ lib, pkgs, ... }:
{
  services.nixaiosLogFat.enable = lib.mkForce true;

  # A ISO de release nao inclui llama.cpp (engine=cpu-echo). A imagem HW inclui.
  environment.systemPackages = [ pkgs.llama-cpp ];
  systemd.services.nosd.path = lib.mkAfter [ pkgs.llama-cpp ];

  # Permite o download no firstboot auto (senao --no-download).
  environment.variables.NOSD_FIRSTBOOT_DOWNLOAD = "1";

  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "NOSD_BOOT_MODEL_REPO=unsloth/Qwen3.5-0.8B-GGUF"
    "NOSD_BOOT_MODEL_FILE=Qwen3.5-0.8B-Q4_K_M.gguf"
    "NOSD_BOOT_MODEL_LOCAL=active-qwen3.5-0.8b-q4_k_m.gguf"
    "NOSD_BOOT_MODEL_ONLY=1"
    # 262k de contexto aloca KV enorme e atrasa/estoura o health de 30s.
    "NOSD_LLAMA_CTX=4096"
  ];
}
