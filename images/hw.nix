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
{ lib, pkgs, self, ... }:
{
  services.nixaiosLogFat.enable = lib.mkForce true;

  # 0.7-alpha1: todos os GGUFs pinados viajam na imagem (offline). O brain
  # default continua 0.8B Q4; os restantes ficam disponíveis para
  # brain.plan/accept (HITL; 9B exige ring) ou model-load explícito.
  systemd.tmpfiles.rules = lib.mkAfter [
    "L+ /var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf - - - - ${self.packages.${pkgs.system}.qwen35-08b-q4km}"
    "L+ /var/lib/nosd/models/Qwen3.5-0.8B-Q8_0.gguf - - - - ${self.packages.${pkgs.system}.qwen35-08b-q8}"
    "L+ /var/lib/nosd/models/Qwen3.5-4B-Q4_K_M.gguf - - - - ${self.packages.${pkgs.system}.qwen35-4b-q4km}"
    "L+ /var/lib/nosd/models/Qwen3.5-9B-Q4_K_M.gguf - - - - ${self.packages.${pkgs.system}.qwen35-9b-q4km}"
  ];

  # Seed offline: NIXAIOSLOG/models/*.gguf -> /var/lib/nosd/models antes do nosd.
  # Com o GGUF no pendrive a inferencia funciona sem rede no 1o boot.
  services.nixaiosLogFat.seedModels = true;

  # Wi-Fi por pendrive (atalho): NIXAIOSLOG/wifi.conf. O Q1 interactivo
  # continua a permitir escolher SSID/senha no tty1.
  services.nixaiosLogFat.seedWifi = true;

  # Metal: questionario Q1-Q5 interactivo no tty1, antes do transmute/GUI.
  # O wrapper (transmute-stage.nix) so dispara firstboot em ttyS*/hvc quando
  # AUTO=1, para as perguntas nao ficarem presas no getty serial.
  services.nixaiosTransmute.firstbootAuto = lib.mkForce false;

  # HW real: a primeira interface grafica e o ORB cinematic em Chromium
  # (--kiosk = fullscreen) sob cage. O Slint continua fallback se o renderer
  # sair/GPU falhar. A ISO de release/QEMU mantem uiMode=slint (iso.nix).
  services.jarbasCage.uiMode = lib.mkForce "cinematic";

  # Boot visivel no ecra: tty0 como console primario, ttyS0 secundario.
  boot.kernelParams = lib.mkAfter [ "console=tty0" ];

  # A ISO de release nao inclui llama.cpp (engine=cpu-echo). A imagem HW inclui.
  environment.systemPackages = [ pkgs.llama-cpp ];
  systemd.services.nosd.path = lib.mkAfter [ pkgs.llama-cpp ];

  # Fallback do modo auto (headless): permite download em vez de --no-download.
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
