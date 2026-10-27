# Anel Cortex→Nix: NOSD_NIX_AUTO_SWITCH + apply root (staging → ai-managed.nix → switch).
# nosd (user) nunca corre nixos-rebuild; só escreve nix-apply.request.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nosdNixRing;
  applyScript = pkgs.writeShellScript "nosd-nix-apply" ''
    set -euo pipefail
    DATA="''${NOSD_DATA_DIR:-/var/lib/nosd}"
    REQ="$DATA/nix-apply.request"
    GEN_FILE="$DATA/last-generation.txt"
    LOG_FAT="/mnt/nixaios-log/nix-apply.log"
    LOG_LOCAL="$DATA/nix-apply.log"
    BUILD_VM_FIRST=${if cfg.buildVmFirst then "1" else "0"}
    log() {
      echo "[$(date -Is 2>/dev/null || date)] $*" | tee -a "$LOG_LOCAL" >/dev/null
      if [ -d /mnt/nixaios-log ] && mountpoint -q /mnt/nixaios-log 2>/dev/null; then
        echo "[$(date -Is 2>/dev/null || date)] $*" >> "$LOG_FAT" || true
      fi
    }
    write_generation() {
      if command -v nix-env >/dev/null 2>&1; then
        ${pkgs.nix}/bin/nix-env --list-generations -p /nix/var/nix/profiles/system 2>/dev/null \
          | tail -n 1 > "$GEN_FILE" || true
      fi
      if [ ! -s "$GEN_FILE" ] && [ -L /run/current-system ]; then
        readlink -f /run/current-system > "$GEN_FILE" || true
      fi
      chown nosd:nosd "$GEN_FILE" 2>/dev/null || chmod 0644 "$GEN_FILE" || true
      log "generation recorded $(cat "$GEN_FILE" 2>/dev/null || echo unknown)"
    }
    if [ ! -f "$REQ" ]; then
      exit 0
    fi
    staging=$(grep '^staging=' "$REQ" | head -1 | cut -d= -f2-)
    target=$(grep '^target=' "$REQ" | head -1 | cut -d= -f2-)
    target="''${target:-/etc/nixos/ai-managed.nix}"
    if [ -z "$staging" ] || [ ! -f "$staging" ]; then
      log "FAIL missing staging"
      rm -f "$REQ"
      exit 1
    fi
    if ! ${pkgs.nix}/bin/nix-instantiate --parse "$staging" >/dev/null 2>&1; then
      log "FAIL parse $staging"
      rm -f "$REQ"
      exit 1
    fi
    mkdir -p "$(dirname "$target")"
    cp -f "$staging" "$target"
    chmod 0640 "$target" || true
    chown root:nosd "$target" || true
    log "copied $staging → $target"
    if command -v nixos-rebuild >/dev/null 2>&1; then
      if [ "$BUILD_VM_FIRST" = "1" ]; then
        log "build-vm (pre-switch)"
        if ! nixos-rebuild build-vm 2>&1 | tee -a "$LOG_LOCAL"; then
          log "build-vm FAIL — switch aborted"
          rm -f "$REQ"
          exit 1
        fi
        log "build-vm OK"
      fi
      if nixos-rebuild switch 2>&1 | tee -a "$LOG_LOCAL"; then
        log "switch OK"
        write_generation
      else
        log "switch FAIL — try: systemctl start nosd-nix-rollback  OR  nixos-rebuild --rollback"
        rm -f "$REQ"
        exit 1
      fi
    else
      log "nixos-rebuild absent — staging copied only"
    fi
    rm -f "$REQ"
    log "done"
  '';
  rollbackScript = pkgs.writeShellScript "nosd-nix-rollback" ''
    set -euo pipefail
    DATA="''${NOSD_DATA_DIR:-/var/lib/nosd}"
    REQ="$DATA/nix-rollback.request"
    LOG_LOCAL="$DATA/nix-rollback.log"
    log() { echo "[$(date -Is 2>/dev/null || date)] $*" | tee -a "$LOG_LOCAL"; }
    if [ ! -f "$REQ" ]; then
      exit 0
    fi
    log "rollback requested"
    if command -v nixos-rebuild >/dev/null 2>&1; then
      if nixos-rebuild --rollback 2>&1 | tee -a "$LOG_LOCAL"; then
        log "rollback OK"
      else
        log "rollback FAIL"
        rm -f "$REQ"
        exit 1
      fi
    else
      log "nixos-rebuild absent"
      rm -f "$REQ"
      exit 1
    fi
    rm -f "$REQ"
    log "done"
  '';
in
{
  options.services.nosdNixRing = {
    enable = lib.mkEnableOption "Cortex Nix ring (auto-switch via root helper)";
    autoSwitch = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Export NOSD_NIX_AUTO_SWITCH=1 to nosd. Default false (NX.0 prod honesty).
        Lab ISO scripts may set services.nosdNixRing.autoSwitch = true.
      '';
    };
    requireHitlConfirm = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Export NOSD_NIX_REQUIRE_HITL=1 — Cortex must confirm before apply request";
    };
    buildVmFirst = lib.mkOption {
      type = lib.types.bool;
      # NX.1 aceite: build-vm antes de switch em prod; lab pode desligar.
      default = true;
      description = "Run nixos-rebuild build-vm before switch; abort apply on failure (NX.1)";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.paths.nosd-nix-apply = {
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = "/var/lib/nosd/nix-apply.request";
        Unit = "nosd-nix-apply.service";
      };
    };

    systemd.services.nosd-nix-apply = {
      description = "Apply nosd staging → ai-managed.nix + nixos-rebuild switch";
      after = [ "local-fs.target" "nosd.service" ];
      # R-01: nixos-rebuild/mountpoint não estão no defaultPath.
      path = [ config.system.build.nixos-rebuild pkgs.nix pkgs.util-linux ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${applyScript}";
        Nice = 5;
      };
    };

    # Rollback: touch /var/lib/nosd/nix-rollback.request → root oneshot (HITL path)
    systemd.paths.nosd-nix-rollback = {
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = "/var/lib/nosd/nix-rollback.request";
        Unit = "nosd-nix-rollback.service";
      };
    };

    systemd.services.nosd-nix-rollback = {
      description = "nixos-rebuild --rollback when nix-rollback.request exists";
      path = [ config.system.build.nixos-rebuild pkgs.nix pkgs.util-linux ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${rollbackScript}";
        Nice = 5;
      };
    };

    systemd.services.nosd.serviceConfig.Environment = lib.mkAfter (
      [ "NOSD_AI_MANAGED_PATH=/etc/nixos/ai-managed.nix" ]
      ++ lib.optional cfg.autoSwitch "NOSD_NIX_AUTO_SWITCH=1"
      ++ lib.optional cfg.requireHitlConfirm "NOSD_NIX_REQUIRE_HITL=1"
    );

    users.users.nosd.extraGroups = lib.mkAfter [ "networkmanager" ];
    networking.networkmanager.enable = lib.mkDefault true;
    environment.systemPackages = [
      pkgs.networkmanager
      (pkgs.writeShellScriptBin "nosd-nix-rollback-request" ''
        set -euo pipefail
        touch /var/lib/nosd/nix-rollback.request
        echo "rollback request → systemctl start nosd-nix-rollback (or wait for path unit)"
      '')
    ];
  };
}
