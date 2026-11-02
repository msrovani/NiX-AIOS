# Imagem para teste em HARDWARE real (nao e a ISO de release).
#
# Liga o export de logs para uma particao FAT32 com LABEL=NIXAIOSLOG:
#  - journal-boot.log / journal-follow.log (nosd, jarbas-*, hotswap)
#  - firstboot.log / hotswap.log / jarbas-cage.log (copias periodicas)
#  - dmesg-boot.log e hw-info.log (lspci/lsusb/uname) para analise pos-teste
# Frota metal: mesh UDP aberto (notes Wi‑Fi + PC AMD wired na mesma LAN).
# ISO de release continua com firewall fechado (images/iso.nix).
#
# Modelo de arranque: Qwen3.5-0.8B (GGUF Q4_K_M, Apache-2.0) baixado no first
# boot via transmute (HITL) para /var/lib/nosd/models. O repo canonico
# `Qwen/Qwen3.5-0.8B` e safetensors (VLM) e nao serve ao llama.cpp; usamos o
# GGUF do unsloth. Texto apenas (vision/mmproj fica para depois).
{ lib, pkgs, self, ... }:
{
  services.nixaiosLogFat.enable = lib.mkForce true;

  # Mesh frota-3: HB UDP 19000 (Wi‑Fi↔ethernet). TCP 19001 = jobs opcional.
  networking.firewall.allowedUDPPorts = lib.mkAfter [ 19000 ];
  networking.firewall.allowedTCPPorts = lib.mkAfter [ 19001 ];

  # HW slim: S1 0.8B Q4 + S2 4B Q6_K + STT (ggml-tiny) + TTS (piper pt_BR).
  # Sem Q8/4B-Q4/9B na imagem (upgrades via brain.plan + rede/seed).
  systemd.tmpfiles.rules = lib.mkAfter [
    "L+ /var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf - - - - ${self.packages.${pkgs.system}.qwen35-08b-q4km}"
    "L+ /var/lib/nosd/models/Qwen3.5-4B-Q6_K.gguf - - - - ${self.packages.${pkgs.system}.qwen35-4b-q6k}"
    # STT: whisper-cli precisa do modelo; sem ficheiro = ModelAbsent no metal.
    "L+ /var/lib/nosd/models/ggml-tiny.bin - - - - ${self.packages.${pkgs.system}.ggml-tiny}"
    # TTS: piper lê <voice>.onnx + <voice>.onnx.json no models_dir.
    "L+ /var/lib/nosd/models/pt_BR-faber-medium.onnx - - - - ${self.packages.${pkgs.system}.piper-pt-br-faber-medium-onnx}"
    "L+ /var/lib/nosd/models/pt_BR-faber-medium.onnx.json - - - - ${self.packages.${pkgs.system}.piper-pt-br-faber-medium-json}"
    # Binário standalone (não pkgs.piper-tts: pyopenjtalk check quebra em nixpkgs).
    # nos-voice: find_piper_cli → /var/lib/nosd/models/piper/piper
    "L+ /var/lib/nosd/models/piper - - - - ${self.packages.${pkgs.system}.piper-standalone}"
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
  # Piper vem via tmpfiles → /var/lib/nosd/models/piper (standalone), não nixpkgs.
  environment.systemPackages = [ pkgs.llama-cpp ];
  systemd.services.nosd.path = lib.mkAfter [ pkgs.llama-cpp ];

  # Fallback do modo auto (headless): permite download em vez de --no-download.
  environment.variables.NOSD_FIRSTBOOT_DOWNLOAD = "1";

  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "NOSD_BOOT_MODEL_REPO=unsloth/Qwen3.5-0.8B-GGUF"
    "NOSD_BOOT_MODEL_FILE=Qwen3.5-0.8B-Q4_K_M.gguf"
    "NOSD_BOOT_MODEL_LOCAL=active-qwen3.5-0.8b-q4_k_m.gguf"
    "NOSD_BOOT_MODEL_ONLY=1"
    # Contexto automático: sem `NOSD_LLAMA_CTX` fixo, o llama-server --fit
    # escolhe o maior ctx/NGL que cabe (mínimo NOSD_LLAMA_FIT_CTX).
    "NOSD_LLAMA_FIT_CTX=2048"
  ];
}
