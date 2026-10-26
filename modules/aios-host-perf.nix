# Ajustes de host para inferência local (prompt Minimal Image — só o viável).
# Default seguro: swappiness/governor/limits no nosd. Hugepages 1G = opt-in.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.aiosHostPerf;
in
{
  options.services.aiosHostPerf = {
    enable = lib.mkEnableOption "Sysctl/governor/limits para nosd (ISO remig)";
    swappiness = lib.mkOption {
      type = lib.types.ints.u8;
      default = 0;
      description = "vm.swappiness (0 = preferir RAM para modelos)";
    };
    performanceGovernor = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "powerManagement.cpuFreqGovernor = performance";
    };
    # 1 GiB × N — NÃO default: reserva física que bricka laptops ≤8 GiB.
    hugepages1g = lib.mkOption {
      type = lib.types.ints.u8;
      default = 0;
      description = "boot.kernelParams hugepagesz=1G hugepages=N (0 = off)";
    };
  };

  config = lib.mkIf cfg.enable {
    boot.kernel.sysctl."vm.swappiness" = lib.mkDefault cfg.swappiness;

    powerManagement.cpuFreqGovernor = lib.mkIf cfg.performanceGovernor (
      lib.mkDefault "performance"
    );

    boot.kernelParams = lib.mkIf (cfg.hugepages1g > 0) [
      "hugepagesz=1G"
      "hugepages=${toString cfg.hugepages1g}"
      "default_hugepagesz=1G"
    ];

    # Já existe nr_hugepages 2 MiB em neural-os; não conflitar se 1G off.
  };
}
