# nosd-early — IA no early boot (prompt mestre B.1): observe + BOOT SCORE pós-sysinit.
# Honesty: NÃO carrega AgentJev/GGUF na ISO (S1.2 REJECT) — S1 = keyword até LOO runtime.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nosdEarly;
  earlyScript = pkgs.writeShellScript "nosd-early-boot" ''
    set -euo pipefail
    SOCK="''${NOSD_SOCKET:-/run/nosd/nosd.sock}"
    DATA="''${NOSD_DATA_DIR:-/var/lib/nosd}"
    CTL="${pkgs.nosd}/bin/nosctl"
    mkdir -p "$DATA"
    export NOSD_S1_BACKEND="''${NOSD_S1_BACKEND:-keyword}"

    # Espera socket (nosd After=local-fs; early After=nosd)
    for i in $(seq 1 60); do
      if [ -S "$SOCK" ] || [ -n "''${NOSD_TCP:-}" ]; then break; fi
      sleep 0.25
    done

    echo "=== nosd-early BOOT $(date -Is 2>/dev/null || date) s1=$NOSD_S1_BACKEND ==="

    OBS_OUT="$DATA/observe-early.json"
    SCORE_OUT="$DATA/boot-score.json"
    if $CTL --socket "$SOCK" observe >"$OBS_OUT" 2>/tmp/nosd-early-observe.err; then
      echo "observe.snapshot OK → $OBS_OUT"
    else
      echo "observe FAIL (nosd ainda a subir?)" >&2
      cat /tmp/nosd-early-observe.err >&2 || true
    fi

    if $CTL --socket "$SOCK" score >"$SCORE_OUT" 2>/tmp/nosd-early-score.err; then
      echo "BOOT SCORE OK → $SCORE_OUT"
      # Linha legível no journal (dmesg AIOS)
      echo "BOOT_SCORE $(tr -d '\n' < "$SCORE_OUT" | head -c 500)"
    else
      echo "score FAIL" >&2
      cat /tmp/nosd-early-score.err >&2 || true
    fi

    # s529: sem FAT — artefactos só em $DATA (/var/lib/nosd)
    echo "early: score/observe em $DATA (FAT OFF)"

    echo "nosd-early: S1=keyword (AgentJev mmap ISO = REJECT; fetch runtime HITL)"
    echo "=== nosd-early DONE ==="
  '';
in
{
  options.services.nosdEarly = {
    enable = lib.mkEnableOption "nosd early boot: observe + BOOT SCORE after sysinit";
  };

  config = lib.mkIf cfg.enable {
    # Default S1 honesty até LOO AgentJev
    environment.variables.NOSD_S1_BACKEND = lib.mkDefault "keyword";

    systemd.services.nosd-early-score = {
      description = "NixAIOS early boot: OBSERVE_SNAPSHOT + BOOT SCORE (pre-getty)";
      after = [ "sysinit.target" "local-fs.target" "nosd.service" ];
      requires = [ "nosd.service" ];
      wants = [ "nosd.service" ];
      # Antes do login TUI / cage — cockpit IA cedo
      before = [ "getty@tty1.service" "jarbas-cage.service" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        # Root: escreve /var/lib/nosd + optional FAT; nosctl fala com nosd
        User = "root";
        Environment = [
          "NOSD_SOCKET=/run/nosd/nosd.sock"
          "NOSD_DATA_DIR=/var/lib/nosd"
          "NOSD_S1_BACKEND=keyword"
        ];
        ExecStart = "${earlyScript}";
      };
    };
  };
}
