# images/hw-cuda-legacy.nix — ISO opcional Pascal (GTX 1050) com driver 580.xx.
# flake importa hw-cuda.nix antes deste ficheiro; aqui so troca o pacote.
{ config, lib, ... }:
{
  hardware.nvidia.package = lib.mkForce (
    config.boot.kernelPackages.nvidiaPackages.legacy_580
      or config.boot.kernelPackages.nvidiaPackages.legacy_470
  );
}
