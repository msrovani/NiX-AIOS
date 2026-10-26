# Diagnóstico stick — UI → prompt (s515+)

## Versão no stick

No prompt (tty1, user `nixos`):

```bash
blkid | grep -i nixaios
# esperado: LABEL="NIXAIOS515"
cat /etc/os-release | head
```

Host Windows (admin): `python nixaios/scripts/check_stick_volid.py`  
→ `VERDICT=STICK_IS_S515` se PVD = ISO Downloads.

## Onde está o log?

ISO **live** = journal em **RAM** (não grava no pendrive por omissão).

```bash
# estado
systemctl status nosd jarbas-hub jarbas-cage jarbas-show-vt --no-pager

# causa da queda da UI
journalctl -u jarbas-cage -u nosd -b --no-pager | tail -120

# ficheiro extra (s516+): /run/jarbas-cage.log
cat /run/jarbas-cage.log 2>/dev/null | tail -80

# orb lab vivo?
curl -sf -o /dev/null -w '%{http_code}\n' http://127.0.0.1:7701/
```

Gravar num USB de dados (se montado) ou copiar para host:

```bash
journalctl -u jarbas-cage -u nosd -b --no-pager > /tmp/jarbas-boot.log
# se tiver partição rw: cp /tmp/jarbas-boot.log /mnt/...
```

## Sintoma “mouse → prompt” / Alt+F7

| VT | Papel |
|----|--------|
| **Alt+F1** | shell autologin `nixos` (rescue) — o “prompt” |
| **Alt+F7** | cage+Chromium; se o serviço morreu, ecrã vazio e volta a parecer “reinício” |

`StartLimitBurst` esgotado → cage **para** → só fica o prompt no F1.

```bash
sudo systemctl reset-failed jarbas-cage
sudo systemctl restart jarbas-cage
# depois Alt+F7
```
