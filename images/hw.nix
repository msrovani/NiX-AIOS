# Imagem para teste em HARDWARE real (nao e a ISO de release).
#
# Liga o export de logs para uma particao FAT32 com LABEL=NIXAIOSLOG:
#  - journal-boot.log / journal-follow.log (nosd, jarbas-*, hotswap)
#  - firstboot.log / hotswap.log / jarbas-cage.log (copias periodicas)
#  - dmesg-boot.log e hw-info.log (lspci/lsusb/uname) para analise pos-teste
# Nao abre SSH nem firewall; comportamento de rede igual a ISO de release.
{ lib, ... }:
{
  services.nixaiosLogFat.enable = lib.mkForce true;
}
