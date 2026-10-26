# Operações — ISO, firstboot, metal

## Build ISO 0.5.1

```bash
bash scripts/build-iso-050.sh
```

- Volid: `NIXAIOS051`
- FAT log: **desligado** (`services.nixaiosLogFat.enable = false`)
- Guard S1.2 corre após copy
- Persistência na live: `/var/lib/nosd` (volátil até install/overlay)

Alias legado: `scripts/build-iso-s529.sh` → redirecciona para 050.

## Firstboot (tty1)

Fluxo (`nos-firstboot`):

1. Rede (Wi‑Fi / cabo / offline)  
2. Idioma/voz  
3. Teclado  
4. Fuso → grava `host-configuration.nix` (sem `nixos-rebuild switch` na live)  
5. Confirmar → `nosctl transmute --confirm` → hotswap JARBAS  

Resume: `/var/lib/nosd/firstboot.step`.  
Done: `/var/lib/nosd/firstboot.done`.  
Falhas GUI: `firstboot.gui_failed`.

### Pós-Q5 — permissões

0.5.0 mitiga EACCES:

- Modelos em `data_dir/models` por omissão.
- Erros do transmute impressos no tty (`/tmp/nos-transmute-out.json`).
- Cage não reaperta `chmod 750` em `/var/lib/nosd`.

## Serviços

| Unit | Função |
|------|--------|
| `nosd.service` | Daemon (Type=notify) |
| `nosd-early-score.service` | observe + boot-score.json |
| `nosd-hotswap-aios.service` | para DM → sobe cage |
| `jarbas-cage.service` | Wayland kiosk tty7 |
| `nosd-nix-apply.service` | apply root (HITL) |

```bash
journalctl -u nosd -u jarbas-cage -u nosd-hotswap-aios -b --no-pager
nosctl --socket /run/nosd/nosd.sock health
nosctl --socket /run/nosd/nosd.sock score
```

## Metal USB

1. Rufus **DD Image mode**.  
2. Se emergency: Options → Copy ISO to RAM.  
3. QEMU PASS ≠ metal PASS.  
4. DRM: `ls -la /dev/dri`; cage falhou ≠ permissão transmute.  
5. Recuperar GUI: `sudo nos-start-gui` ou `systemctl start jarbas-cage; chvt 7`.

Preflight: `scripts/metal-iso-deep-preflight.sh`.  
Handoff Windows: `scripts/metal-usb-handoff.ps1`.

## FAT NIXAIOSLOG (opt-in)

Default OFF. Para reactivar num perfil lab:

- `NIXAIOS_USE_FAT_PERSIST=1` no ambiente nosd.
- Prep mount: dirs `nosd-persist/models` ownership `nosd`.
- Ver legado [`stick-log-fat32.md`](stick-log-fat32.md).

## CUDA

- Host `nvidia-smi` OBSERVED ≠ engine CUDA no flake default.
- Opt-in: `nix build .#llama-cpp-cuda` + `NOSD_LLAMA_NGL`.
- Smoke: `scripts/smoke-cuda-honesty.sh`.
