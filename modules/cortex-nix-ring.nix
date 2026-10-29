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
    # S-06/S-04: ring desarmado por omissao. O pedido vive num diretorio
    # world-writable, por isso o helper NAO pode confiar so no ficheiro.
    if [ "''${NOSD_NIX_RING:-0}" != "1" ]; then
      log "ring Nix desarmado (NOSD_NIX_RING != 1) - pedido ignorado"
      rm -f "$REQ"
      exit 0
    fi
    exec 9>"$DATA/nix-apply.lock"
    if ! ${pkgs.util-linux}/bin/flock -n 9; then
      log "apply ja em curso (lock) - a sair"
      exit 0
    fi
    staging=$(grep '^staging=' "$REQ" | head -1 | cut -d= -f2-)
    # R-04: target e FIXO; nunca vem do pedido.
    target=/etc/nixos/ai-managed.nix
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
    case "$staging" in
      "$DATA"/*) ;;
      *)
        log "FAIL staging fora de $DATA ($staging) - recusado"
        rm -f "$REQ"
        exit 1
        ;;
    esac
    mkdir -p "$(dirname "$target")"
    cp -f "$staging" "$target"
    chmod 0640 "$target" || true
    chown root:nosd "$target" || true
    log "copied $staging → $target"
    CONFIRM="$DATA/nix-confirm.request"
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
      # G3.2: duas fases. Fase 1 (sem confirm): copia + build-vm + test —
      # o sistema ACTIVA o novo config em runtime mas a geração de BOOT
      # continua intacta; o switch só corre na fase 2, que exige
      # `nix-confirm.request` (TTL 600 s) escrito pelo `nix.confirm` HITL.
      if [ "''${NOSD_NIX_TWO_PHASE:-1}" = "1" ] && [ ! -f "$CONFIRM" ]; then
        log "fase 1: nixos-rebuild test (boot default intacta; switch exige nix.confirm)"
        if ! ${pkgs.coreutils}/bin/timeout 900 nixos-rebuild test 2>&1 | tee -a "$LOG_LOCAL"; then
          log "test FAIL — switch abortado"
          rm -f "$REQ"
          exit 1
        fi
        rm -f "$REQ"
        log "test OK — aguarda nix.confirm (2º pedido, TTL 600s)"
        exit 0
      fi
      if [ "''${NOSD_NIX_TWO_PHASE:-1}" = "1" ]; then
        cts=$(grep '^ts=' "$CONFIRM" | head -1 | cut -d= -f2)
        now=$(date +%s)
        if ! [[ "$cts" =~ ^[0-9]+$ ]]; then
          log "FAIL confirm sem ts válido"
          rm -f "$REQ" "$CONFIRM"
          exit 1
        fi
        if [ $((now - cts)) -gt 600 ]; then
          log "FAIL confirm expirado (TTL 600s) — refaça nix.confirm"
          rm -f "$REQ" "$CONFIRM"
          exit 1
        fi
        log "fase 2: confirm válido (ts=$cts) — switch"
      fi
      if ${pkgs.coreutils}/bin/timeout 900 nixos-rebuild switch 2>&1 | tee -a "$LOG_LOCAL"; then
        log "switch OK"
        write_generation
      else
        log "switch FAIL — try: systemctl start nosd-nix-rollback  OR  nixos-rebuild --rollback"
        rm -f "$REQ" "$CONFIRM"
        exit 1
      fi
    else
      log "nixos-rebuild absent — staging copied only"
    fi
    rm -f "$REQ" "$CONFIRM"
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
    # S-06: rollback tambem so com o ring armado (senao um user em
    # /var/lib/nosd 1777 podia forcar geracoes antigas).
    if [ "''${NOSD_NIX_RING:-0}" != "1" ]; then
      log "ring Nix desarmado (NOSD_NIX_RING != 1) - rollback ignorado"
      rm -f "$REQ"
      exit 0
    fi
    log "rollback requested"
    if command -v nixos-rebuild >/dev/null 2>&1; then
      if ${pkgs.coreutils}/bin/timeout 600 nixos-rebuild --rollback 2>&1 | tee -a "$LOG_LOCAL"; then
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
    armRing = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Arma o ring Nix (S-06): exporta NOSD_NIX_RING=1 para os helpers root
        `nosd-nix-apply`/`nosd-nix-rollback`. Enquanto false, os helpers
        ignoram pedidos: /var/lib/nosd e world-writable na ISO e qualquer
        user poderia escrever um pedido. So ligar com S-04 + R-03/R-04 verdes.
      '';
    };
    buildVmFirst = lib.mkOption {
      type = lib.types.bool;
      # NX.1 aceite: build-vm antes de switch em prod; lab pode desligar.
      default = true;
      description = "Run nixos-rebuild build-vm before switch; abort apply on failure (NX.1)";
    };
    twoPhase = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        G3.2: apply em duas fases — fase 1 copia staging + build-vm +
        `nixos-rebuild test` (boot default intacta) e para; o `switch` só
        corre na fase 2, que exige `nix-confirm.request` (escrito por
        `nix.confirm` HITL, TTL 600 s). Anti-brick por omissão.
      '';
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
      # R-01: nixos-rebuild/mountpoint não estão no defaultPath. T3.3 (VM):
      # com o ring ARMADO, o helper roda grep/cut — sem gnugrep o apply
      # morria em exit 127 ANTES da fase 1 (latente: só provado com armed).
      path = [ config.system.build.nixos-rebuild pkgs.nix pkgs.util-linux pkgs.coreutils pkgs.gnugrep ];
      environment = lib.optionalAttrs cfg.armRing {
        NOSD_NIX_RING = "1";
        NOSD_NIX_TWO_PHASE = if cfg.twoPhase then "1" else "0";
      };
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
      path = [ config.system.build.nixos-rebuild pkgs.nix pkgs.util-linux pkgs.coreutils ];
      environment = lib.optionalAttrs cfg.armRing { NOSD_NIX_RING = "1"; };
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
      # T3.3 (apanhado pelo nixaios-ring com stub): o kill switch do DAEMON
      # (S-06) também tem de armar com armRing — sem isto, ring armado no
      # helper morria em NIX_RING_DISABLED no próprio nosd.
      ++ lib.optional cfg.armRing "NOSD_NIX_RING=1"
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
