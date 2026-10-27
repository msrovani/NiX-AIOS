# Offline ISO — firstboot + Slint HUD refinado (cage/iGPU, sem Chromium default).
# Build: scripts/build-iso.sh (volid NIXAIOS051 = release 0.5.x; workspace 0.5.2+)
# POLICY: NÃO embutir modelos/firmware/voice blobs — runtime fetch deliberado
# Cortex→Hermes→JARBAS (nos-hermes::blob_deliberate / model.fetch|firmware.fetch).
{ pkgs, lib, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/installer/cd-dvd/installation-cd-minimal.nix"
  ];

  isoImage.makeEfiBootable = true;
  isoImage.makeUsbBootable = true;
  isoImage.volumeID = "NIXAIOS051";

  # R-09: QEMU headless precisa de console serial; metal continua em tty0.
  boot.kernelParams = lib.mkAfter [ "console=ttyS0,115200n8" ];

  # 0.5.1: FAT OFF — persistência só /var/lib/nosd (evita EACCES pós-Q5 no metal).
  services.nixaiosLogFat.enable = false;
  services.nixaiosTransmute.enable = true;

  # Ganha sobre tmpfiles 0750 do neural-os.nix (live: nixos + nosd escrevem).
  systemd.tmpfiles.rules = lib.mkAfter [
    "d /var/lib/nosd 1777 nosd nosd -"
    "d /var/lib/nosd/models 1777 nosd nosd -"
    "d /var/lib/nosd/nsgdb 1777 nosd nosd -"
  ];

  # NX.0: autoSwitch off on prod ISO; lab scripts (e.g. build-iso-*.sh) may override true.
  services.nosdNixRing = {
    enable = true;
    autoSwitch = false;
    requireHitlConfirm = true;
  };

  networking.hostName = "nixaios";
  networking.networkmanager.enable = true;
  services.openssh.enable = lib.mkForce false;

  hardware.graphics.enable = true;
  hardware.graphics.enable32Bit = lib.mkDefault false;

  # Autologin tty1 → nos-firstboot (transmute-stage). Nao forçar null.
  services.getty.autologinUser = lib.mkForce "nixos";

  services.nosd = {
    enable = true;
    dataDir = "/var/lib/nosd";
    hermesTickSecs = 2;
  };

  # Prompt Minimal Image (triagem): swappiness/governor; hugepages 1G fica 0.
  services.aiosHostPerf = {
    enable = true;
    swappiness = 0;
    performanceGovernor = true;
    hugepages1g = 0;
  };

  services.nosdEarly.enable = true;

  services.jarbasCage = {
    enable = true;
    nosdPackage = pkgs.nosd;
    drmDevice = ""; # pickDrm → Intel iGPU
    tty = "tty7";
    autoStart = false;
    hubFallback = true;
    # s528: Slint HUD refinado (iGPU cage); Chromium só se uiMode=cinematic
    uiMode = "slint";
  };

  services.nosdAiManaged = {
    enable = true;
  };

  system.activationScripts.nosdModels = ''
    mkdir -p /var/lib/nosd/models
    mkdir -p /var/lib/nosd/nsgdb
    # 0.6: /run/nosd/user (nao /run/user/nosd: tmpfs do logind esconde).
    mkdir -p /run/nosd/user
    # Live ISO: nixos + nosd precisam escrever (sem FAT)
    chmod 1777 /var/lib/nosd /var/lib/nosd/models /var/lib/nosd/nsgdb || true
    chown nosd:nosd /run/nosd/user || true
    chmod 700 /run/nosd/user || true
  '';

  environment.systemPackages = with pkgs; [
    pciutils
    vulkan-tools
    vulkan-loader
    mesa
    cage
    nosd
    chromium
    v4l-utils
    dejavu_fonts
    fontconfig
  ];

  boot.kernelModules = [ "uvcvideo" ];
  nix.settings.substituters = lib.mkDefault [ ];
  # Sem GNOME: multi-user + getty; cage sobe no hotswap.
  systemd.defaultUnit = lib.mkOverride 900 "multi-user.target";
}
