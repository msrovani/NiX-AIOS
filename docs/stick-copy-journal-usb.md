# Copiar journal → pen FAT → WSL (opção 1)

## No stick (ainda booted em NixAIOS)

1. Liga um **2.º USB** formatado em **FAT32** (o boot stick é RO).
2. No prompt:

```bash
journalctl -u jarbas-cage -u nosd -b --no-pager > /tmp/jarbas-boot.log

lsblk -o NAME,SIZE,TYPE,FSTYPE,LABEL,TRAN
# escolhe a partição FAT do 2.º pen (ex: sdb1) — NÃO o disco de boot da ISO

sudo mkdir -p /mnt/logusb
sudo mount /dev/sdX1 /mnt/logusb
sudo cp /tmp/jarbas-boot.log /mnt/logusb/jarbas-boot.log
sync
sudo umount /mnt/logusb
```

3. Podes reiniciar para o Windows.

## No Windows / WSL

Pen aparece como letra (ex. `E:`). No WSL:

```bash
ls /mnt/e/jarbas-boot.log   # ajusta a letra
cp /mnt/e/jarbas-boot.log /mnt/c/Users/msrov/Downloads/
tail -120 /mnt/c/Users/msrov/Downloads/jarbas-boot.log
```

Diz no chat quando o ficheiro estiver em Downloads — lemos daí.
