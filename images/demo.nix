# DEMO ONLY — nunca entra na ISO de release.
#
# Configura a mesh sem intervencao manual:
#  - IP 192.168.50.N derivado do MAC da NIC de mesh (52:54:00:12:34:0N);
#  - broadcast de sub-rede (nao 255.255.255.255, que exige rota extra);
#  - firewall aberto (demo isolada) para UDP 19000 / TCP 19001;
#  - nosd com identidade obrigatoria + job bind;
#  - ssh root/nixaios para o host fazer as assercoes (so nesta ISO demo).
{ lib, pkgs, ... }:
{
  networking.firewall.enable = false;

  services.openssh = {
    enable = true;
    settings.PermitRootLogin = lib.mkForce "yes";
    settings.PasswordAuthentication = lib.mkForce true;
  };
  # password com hash no build (initialPassword nao estava a aplicar no live)
  users.users.root.password = "nixaios";

  # A NIC de mesh e identificada pelo MAC; o IP sai do ultimo octeto.
  systemd.services.demo-net = {
    description = "NixAIOS demo: IP de mesh derivado do MAC";
    wantedBy = [ "multi-user.target" ];
    before = [ "nosd.service" ];
    path = [ pkgs.iproute2 pkgs.coreutils pkgs.bash pkgs.util-linux ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      set -eu
      for d in /sys/class/net/*; do
        iface=$(basename "$d")
        [ "$iface" = lo ] && continue
        mac=$(cat "$d/address")
        case "$mac" in
          52:54:00:12:34:0*)
            n=$((16#''${mac##*:}))
            ip link set "$iface" up
            ip addr flush dev "$iface"
            ip addr add "192.168.50.$n/24" dev "$iface"
            ip route replace 255.255.255.255 dev "$iface"
            logger -t demo-net "$iface $mac -> 192.168.50.$n" || true
            ;;
        esac
      done
    '';
  };

  services.nosd.meshBroadcast = lib.mkForce "192.168.50.255:19000";

  systemd.services.nosd = {
    after = [ "demo-net.service" ];
    wants = [ "demo-net.service" ];
    serviceConfig.Environment = lib.mkAfter [
      "NOSD_MESH_REQUIRE_SIG=1"
      "NOSD_JOB_BIND=0.0.0.0:19001"
    ];
  };
}
