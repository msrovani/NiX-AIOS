# Particao FAT32 LABEL=NIXAIOSLOG -> /mnt/nixaios-log (logs texto, legivel no Windows).
# s526: mount robusto (by-label + blkid + maior VFAT nao-EFI) — s525 stick so tinha README.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nixaiosLogFat;
  mnt = cfg.mountPoint;

  ensureMount = pkgs.writeShellScript "nixaios-ensure-log-fat" ''
    set -eu
    MNT="${mnt}"
    LABEL="${cfg.label}"
    mkdir -p "$MNT"
    if mountpoint -q "$MNT" 2>/dev/null; then
      echo "nixaios-log: already mounted $MNT"
      exit 0
    fi

    try_mount() {
      dev="$1"
      [ -n "$dev" ] && [ -b "$dev" ] || return 1
      # Resolver symlink (by-label) para o no real
      real=$(readlink -f "$dev" 2>/dev/null || echo "$dev")
      echo "nixaios-log: trying $dev ($real)"
      ${pkgs.dosfstools}/bin/fsck.vfat -a "$real" 2>/dev/null || true
      if mount -t vfat -o rw,noatime,umask=000,uid=0,gid=0 "$real" "$MNT" 2>/tmp/nixaios-mount.err; then
        echo "nixaios-log: OK $real -> $MNT"
        return 0
      fi
      cat /tmp/nixaios-mount.err 2>/dev/null || true
      mount -t vfat -o rw,noatime,umask=000,uid=0,gid=0,errors=continue "$real" "$MNT" 2>/tmp/nixaios-mount2.err && return 0
      cat /tmp/nixaios-mount2.err 2>/dev/null || true
      return 1
    }

    # 1) by-label (ate 45s — USB lento)
    for i in $(seq 1 45); do
      if [ -e "/dev/disk/by-label/$LABEL" ]; then
        try_mount "/dev/disk/by-label/$LABEL" && break
      fi
      # udev settle
      ${pkgs.systemd}/bin/udevadm settle --timeout=2 2>/dev/null || true
      sleep 1
    done

    if ! mountpoint -q "$MNT" 2>/dev/null; then
      # 2) blkid / findfs LABEL=
      dev=$(${pkgs.util-linux}/bin/blkid -L "$LABEL" 2>/dev/null || true)
      try_mount "$dev" || true
      if ! mountpoint -q "$MNT" 2>/dev/null; then
        dev=$(${pkgs.util-linux}/bin/findfs "LABEL=$LABEL" 2>/dev/null || true)
        try_mount "$dev" || true
      fi
    fi

    if ! mountpoint -q "$MNT" 2>/dev/null; then
      # 3) maior particao vfat >100MB que nao seja EFI/ESP (BOOT/EFI*)
      while IFS= read -r line; do
        dev=$(echo "$line" | awk '{print $1}')
        lab=$(echo "$line" | sed -n 's/.*LABEL="\([^"]*\)".*/\1/p')
        case "$lab" in
          EFI*|BOOT*|ESP*) continue ;;
        esac
        sz=$(${pkgs.coreutils}/bin/blockdev --getsize64 "$dev" 2>/dev/null || echo 0)
        if [ "$sz" -gt 104857600 ]; then
          try_mount "$dev" && break
        fi
      done < <(${pkgs.util-linux}/bin/blkid -t TYPE=vfat 2>/dev/null || true)
    fi

    if mountpoint -q "$MNT" 2>/dev/null; then
      chmod 1777 "$MNT" 2>/dev/null || true
      # Marcador sempre — prova no Windows que o guest escreveu
      STAMP=$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || echo boot)
      echo "NIXAIOS guest mount OK $STAMP host=$(hostname 2>/dev/null || echo ?)" > "$MNT/GUEST_MOUNTED.txt"
      sync
      ls -la "$MNT" || true
      exit 0
    fi

    echo "nixaios-log: FAIL nao montou LABEL=$LABEL" >&2
    ${pkgs.util-linux}/bin/blkid || true
    ls -la /dev/disk/by-label/ 2>/dev/null || true
    exit 1
  '';
in
{
  options.services.nixaiosLogFat = {
    enable = lib.mkEnableOption "NIXAIOSLOG FAT32 log partition";
    label = lib.mkOption {
      type = lib.types.str;
      default = "NIXAIOSLOG";
    };
    mountPoint = lib.mkOption {
      type = lib.types.str;
      default = "/mnt/nixaios-log";
    };
  };

  config = lib.mkIf cfg.enable {
    boot.supportedFilesystems = [ "vfat" ];

    # Tentativa declarativa (nofail); o servico ensure faz o trabalho real.
    fileSystems.${mnt} = {
      device = "/dev/disk/by-label/${cfg.label}";
      fsType = "vfat";
      options = [
        "rw"
        "nofail"
        "noatime"
        "umask=000"
        "gid=0"
        "uid=0"
        "x-systemd.device-timeout=45s"
        "x-systemd.mount-timeout=45s"
      ];
    };

    systemd.tmpfiles.rules = [
      "d ${mnt} 1777 root root -"
      "d /var/lib/nosd 1777 nosd nosd -"
    ];

    systemd.services.nixaios-log-mount = {
      description = "Mount NIXAIOSLOG FAT (robust by-label/blkid)";
      wantedBy = [ "multi-user.target" "local-fs.target" ];
      after = [ "local-fs-pre.target" "systemd-udev-settle.service" ];
      before = [ "nixaios-log-export.service" "nosd.service" ];
      wants = [ "systemd-udev-settle.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${ensureMount}";
        # Nao falhar o boot inteiro — firstboot avisa no tty
        SuccessExitStatus = [ 0 1 ];
      };
    };

    systemd.services.nixaios-log-export = {
      description = "Export journal -> NIXAIOSLOG FAT32";
      wantedBy = [ "multi-user.target" ];
      after = [ "nixaios-log-mount.service" "nosd.service" ];
      wants = [ "nixaios-log-mount.service" ];
      serviceConfig = {
        Type = "simple";
        Restart = "on-failure";
        RestartSec = "10s";
        # tools usadas no script (cp/sleep/dmesg/lspci/lsusb)
        path = with pkgs; [ coreutils util-linux pciutils usbutils ];
        ExecStart = pkgs.writeShellScript "nixaios-log-export" ''
          set -euo pipefail
          MNT="${mnt}"
          # Re-tenta mount se necessario
          if ! mountpoint -q "$MNT" 2>/dev/null; then
            ${ensureMount} || true
          fi
          if ! mountpoint -q "$MNT" 2>/dev/null; then
            echo "nixaios-log-export: FAT ausente — so journal RAM" >&2
            exit 0
          fi
          STAMP=$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || echo boot)
          # retrato de HW para analise pos-teste
          {
            echo "=== nixaios hw $STAMP ==="
            uname -a || true
            cat /proc/cmdline || true
            ${pkgs.pciutils}/bin/lspci -nn || true
            ${pkgs.usbutils}/bin/lsusb || true
          } > "$MNT/hw-info.log" 2>&1 || true
          ${pkgs.util-linux}/bin/dmesg > "$MNT/dmesg-boot.log" 2>&1 || true
          {
            echo "=== nixaios journal dump $STAMP ==="
            ${pkgs.systemd}/bin/journalctl -b -u nosd -u jarbas-cage -u jarbas-hub -u jarbas-show-vt -u nosd-hotswap-aios -u nixaios-log-mount -u nixaios-log-export --no-pager || true
            echo "--- firstboot/hotswap tail ---"
            for f in firstboot.log hotswap.log jarbas-cage.log; do
              [ -f "$MNT/$f" ] && echo "===== $f =====" && tail -n 80 "$MNT/$f" || true
            done
          } > "$MNT/journal-boot.log"
          # firstboot/hotswap/cage escrevem em /tmp e /var/lib/nosd: copia
          # periodica para o FAT (senao perdem-se no reboot).
          (
            while true; do
              for f in /tmp/nos-firstboot.log /var/lib/nosd/firstboot.log \
                       /var/lib/nosd/hotswap.log /var/lib/nosd/jarbas-cage.log; do
                [ -f "$f" ] && cp -f "$f" "$MNT/$(basename "$f")" 2>/dev/null || true
              done
              sync
              sleep 20
            done
          ) &
          sync
          exec ${pkgs.systemd}/bin/journalctl -f -u nosd -u jarbas-cage -u jarbas-hub -u nosd-hotswap-aios -o short-iso \
            >> "$MNT/journal-follow.log"
        '';
      };
    };

    environment.variables.NIXAIOS_LOG_DIR = mnt;
    environment.systemPackages = [ pkgs.util-linux pkgs.dosfstools ];
  };
}
