# AI-managed Nix fragment — CapGate NIX_WRITE + HITL switch
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nosdAiManaged;
in
{
  options.services.nosdAiManaged = {
    enable = lib.mkEnableOption "Import ai-managed.nix allowlist for Cortex proposals";
    path = lib.mkOption {
      type = lib.types.path;
      default = "/etc/nixos/ai-managed.nix";
      description = "Only file nosd may propose writes into (Landlock/allowlist)";
    };
  };

  config = lib.mkIf cfg.enable {
    # Empty default — IA writes staging, never this path directly without HITL
    environment.etc."nixos/ai-managed.nix" = {
      text = ''
        # Managed by nosd CapGate NIX_WRITE after build-vm + HITL approve.
        # Schema allowlist (NX.0): only these top-level keys may appear in proposals:
        #   networking.hostName, services.nosd, services.jarbasCage, services.nosdNixRing,
        #   services.nosdEngine, environment.systemPackages (packages only, no arbitrary strings).
        # Rollback: nosd-nix-rollback-request → nosd-nix-rollback.service
        { ... }: { }
      '';
      mode = "0640";
      user = "root";
      group = "nosd";
    };

    # Helper legado + atalho apply (anel Cortex usa nosd-nix-apply.service)
    environment.systemPackages = [
      (pkgs.writeShellScriptBin "nosd-nix-propose-check" ''
        set -euo pipefail
        STAGING="''${1:-/var/lib/nosd/staging-ai-managed.nix}"
        if [ ! -f "$STAGING" ]; then
          echo "missing staging $STAGING" >&2
          exit 1
        fi
        if command -v nix-instantiate >/dev/null; then
          nix-instantiate --parse "$STAGING" >/dev/null
          echo "parse OK"
        fi
        echo "Cortex ring: nosctl nix-apply  OR  systemctl start nosd-nix-apply"
        echo "rollback: nixos-rebuild --rollback"
      '')
      (pkgs.writeShellScriptBin "nosd-wifi" ''
        set -euo pipefail
        case "''${1:-}" in
          scan) exec nosctl wifi-scan ;;
          connect) shift; exec nosctl wifi-connect "$@" ;;
          *) echo "usage: nosd-wifi scan | connect <ssid> --password <pw>"; exit 1 ;;
        esac
      '')
      (pkgs.writeShellScriptBin "nosd-model-plan" ''
        exec nosctl plan
      '')
    ];
  };
}
