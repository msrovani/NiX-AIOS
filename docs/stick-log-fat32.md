# Log persistente FAT32 (s516+)

## Imagem

Rufus → **Imagem DD** de:

`Downloads\nixaios-minimal-s516-logfat.img`

(não uses só o `.iso` se quiseres a partição de log já criada).

## Partições

| Label | Uso |
|-------|-----|
| `NIXAIOS516` | live boot (ISO9660) |
| `NIXAIOSLOG` | FAT32 ~512 MiB — logs texto |

## Ficheiros (Windows)

Abrir o volume **NIXAIOSLOG**:

- `journal-boot.log` — dump no boot
- `journal-follow.log` — append ao vivo
- `jarbas-cage.log` — cage/chromium
- `README.txt`

## WSL

```bash
# letra do volume NIXAIOSLOG no Windows, ex. E:
cp /mnt/e/journal-boot.log /mnt/c/Users/msrov/Downloads/
cp /mnt/e/jarbas-cage.log /mnt/c/Users/msrov/Downloads/
```
