# Colheita M3b + gate M3b-audio no metal, sem HITL.
# Corre uma vez por boot depois do nosd (e espera cage/voz). Resultados em
# NIXAIOSLOG/boots/<stamp>_<host>/m3b-probe/ (ou /var/lib/nosd se FAT ausente).
# Opt-out: touch /var/lib/nosd/m3b-probe.skip ou NOSD_SKIP_M3B_PROBE=1 no unit.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nixaiosM3bProbe;
  harvestSrc = ../scripts/metal-m3b-harvest.sh;
  gateSrc = ../scripts/gate-boot-to-jarbas.sh;

  harvestBin = pkgs.writeShellScriptBin "nixaios-m3b-harvest" ''
    exec ${pkgs.bash}/bin/bash ${harvestSrc} "$@"
  '';
  gateBin = pkgs.writeShellScriptBin "nixaios-m3b-gate-audio" ''
    export NIXAIOS_GATE_AUDIO=1
    exec ${pkgs.bash}/bin/bash ${gateSrc} --stage M3b-audio "$@"
  '';

  probeScript = pkgs.writeShellScript "nixaios-m3b-probe" ''
    set -euo pipefail
    PATH="${lib.makeBinPath [
      pkgs.bash pkgs.coreutils pkgs.gnugrep pkgs.gnused pkgs.util-linux
      pkgs.alsa-utils pkgs.python3 pkgs.systemd
      config.services.nosd.package
    ]}:$PATH"

    if [ -f /var/lib/nosd/m3b-probe.skip ] || [ "''${NOSD_SKIP_M3B_PROBE:-0}" = "1" ]; then
      echo "nixaios-m3b-probe: skip (flag)"
      exit 0
    fi

    MNT="${config.services.nixaiosLogFat.mountPoint}"
    STAMP=$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || echo boot)
    HOST="$(hostname 2>/dev/null || echo nixaios)"

    # Preferir boots/LATEST do export; senao criar directorio proprio.
    if mountpoint -q "$MNT" 2>/dev/null; then
      if [ -f "$MNT/LATEST" ]; then
        REL="$(tr -d '\r\n' < "$MNT/LATEST")"
        BOOT_DIR="$MNT/$REL"
      else
        BOOT_DIR="$MNT/boots/''${STAMP}_''${HOST}"
      fi
      mkdir -p "$BOOT_DIR"
    else
      BOOT_DIR="/var/lib/nosd/m3b-probe/''${STAMP}_''${HOST}"
      mkdir -p "$BOOT_DIR"
      echo "nixaios-m3b-probe: FAT ausente — a escrever em $BOOT_DIR" >&2
    fi
    OUT="$BOOT_DIR/m3b-probe"
    mkdir -p "$OUT"
    exec > >(tee "$OUT/probe.log") 2>&1
    echo "=== nixaios-m3b-probe $STAMP host=$HOST out=$OUT ==="

    # Esperar socket nosd (fail-closed soft: regista e sai 0).
    SOCK="/run/nosd/nosd.sock"
    for i in $(seq 1 120); do
      if [ -S "$SOCK" ]; then
        break
      fi
      sleep 1
    done
    if [ ! -S "$SOCK" ]; then
      echo "nosd_socket_absent" | tee "$OUT/verdict.txt"
      exit 0
    fi

    # Dar tempo ao cage/PipeWire (voz) sem bloquear o boot com FAIL.
    DELAY="${toString cfg.delaySecs}"
    echo "waiting ''${DELAY}s for cage/voice…"
    sleep "$DELAY"

    export NOSCTL="${config.services.nosd.package}/bin/nosctl"
    export NOSD_SOCKET="$SOCK"
    export NOSD_IPC_TOKEN_FILE="${config.services.nosd.dataDir}/ipc.token"
    export XDG_RUNTIME_DIR_NOSD="/run/nosd/user"

    echo "--- harvest ---"
    ${pkgs.bash}/bin/bash ${harvestSrc} "$OUT/harvest" || true

    echo "--- gate M3b-audio ---"
    set +e
    NIXAIOS_GATE_AUDIO=1 ${pkgs.bash}/bin/bash ${gateSrc} --stage M3b-audio \
      >"$OUT/gate-m3b-audio.txt" 2>&1
    GATE_RC=$?
    set -e
    echo "gate_rc=$GATE_RC" | tee -a "$OUT/gate-m3b-audio.txt"

    # Sumario curto legivel no Windows
    {
      echo "stamp=$STAMP host=$HOST"
      echo "gate_rc=$GATE_RC"
      if [ -f "$OUT/harvest/harvest.log" ]; then
        grep -E '^A1=|^A2=|^A4=|^A6=|play_ok|snd_denied' "$OUT/harvest/harvest.log" | tail -n 20 || true
      fi
      if [ -f "$OUT/gate-m3b-audio.txt" ]; then
        grep -E '^stage=M3b-audio|^gate_summary' "$OUT/gate-m3b-audio.txt" || true
      fi
    } > "$OUT/SUMMARY.txt"
    cp -f "$OUT/SUMMARY.txt" "$BOOT_DIR/m3b-SUMMARY.txt" 2>/dev/null || true
    if mountpoint -q "$MNT" 2>/dev/null; then
      cp -f "$OUT/SUMMARY.txt" "$MNT/m3b-SUMMARY.txt" 2>/dev/null || true
    fi
    sync || true
    echo "=== DONE ==="
    exit 0
  '';
in
{
  options.services.nixaiosM3bProbe = {
    enable = lib.mkEnableOption "auto M3b harvest + gate-audio no metal (sem HITL)";
    delaySecs = lib.mkOption {
      type = lib.types.int;
      default = 90;
      description = "Espera após nosd.socket para cage/PipeWire antes da colheita.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ harvestBin gateBin ];

    systemd.services.nixaios-m3b-probe = {
      description = "NixAIOS M3b metal harvest + gate-audio (no HITL)";
      wantedBy = [ "multi-user.target" ];
      after = [
        "nosd.service"
        "nixaios-log-mount.service"
        "nixaios-log-export.service"
        "jarbas-cage.service"
      ];
      wants = [ "nosd.service" "nixaios-log-mount.service" ];
      # Nao bloquear o arranque: oneshot longo, sempre sucesso no unit.
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        TimeoutStartSec = "600";
        ExecStart = "${probeScript}";
      };
      # path do UNIT (lição: grep/bash absolutos via script; nosctl via PATH)
      path = [
        pkgs.bash
        pkgs.coreutils
        pkgs.gnugrep
        pkgs.alsa-utils
        pkgs.python3
        pkgs.systemd
        config.services.nosd.package
      ];
    };
  };
}
