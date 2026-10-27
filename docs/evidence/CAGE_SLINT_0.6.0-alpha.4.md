# Evidence — M3 HUD Slint em cage/DRM (0.6.0-alpha.4)

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux/qemu` — **OBSERVED** (não metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/9s16g0k86k494np213yivrxmy4ds8p9r-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**SHA-256:** `6b084e2779fc3d73e9054ddd3572b797196a6dd33ca4244849cf2678d5d1af51`  
**Screenshot:** [`qemu-cage-hud-0.6.0-alpha.4.png`](qemu-cage-hud-0.6.0-alpha.4.png)

## Como foi obtido

```bash
# ISO alpha.4 com virtio-gpu (sem VGA) + monitor QEMU para screendump
bash scripts/run-iso-qemu-wsl.sh   # DISPLAY_MODE=serial MEM=4G SMP=2
#   QEMU_EXTRA="-device virtio-gpu-pci -vga none -monitor unix:$MON,server,nowait"
# no guest (auto-login tty1):
sudo systemctl start jarbas-cage
# no host, via monitor:
screendump /tmp/shot.ppm   # convertido para PNG (1280x800)
```

Gate:

```bash
NIXAIOS_QEMU_LOG=... bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
# stage=MQEMU ... layer=DELEGATE claim_status=OBSERVED result=PASS detail=qemu_serial_boot_markers
# stage=MQEMU ... layer=ADAPT    claim_status=OBSERVED result=PASS detail=nosd_start_ok_in_qemu_log
# gate_summary fails=0 stage=MQEMU
```

## Marcadores

```
● jarbas-cage.service - JARBAS kiosk (slint / iGPU cage)
     Active: active (running) since ...; 52s ago
  1441 cage -s -- .../env WINIT_UNIX_BACKEND=wayland WAYLAND_DISPLAY=wayland-0 \
       .../nosd-0.6.0-alpha.4/bin/nos-jarbas --ui --socket /run/nosd/nosd.sock
[backend/drm/drm.c:980] connector Virtual-1: Modesetting with 1280x800 @ 74.994 Hz
jarbas: JARBAS UI start backend=Err(NotPresent) wayland=Ok("wayland-0") display=Ok(":0")
jarbas: JARBAS Slint HUD shown — Alt+F7 / journalctl -u jarbas-cage
```

O screenshot mostra o HUD Slint completo: título JARBAS, `idle · ONLINE`,
`nosd cpu-echo · modelos 0 · peers 0`, linha `NOSD_SCORE ...`, orb animado,
campo de chat e rodapé “Sistema online — orb cinemático”.

## O que prova

- `jarbas-cage` arranca **sem** erros de runtime dir / seatd / tty.
- `cage` toma DRM master em `/dev/dri/card0` (virtio-gpu) e faz modeset.
- O binário da ISO **inclui** `slint-ui` (`cargoBuildFlags`) e desenha pixels
  via Wayland (`WINIT_UNIX_BACKEND=wayland`).
- Fallback `SLINT_BACKEND=winit-software` + `WLR_RENDERER=pixman` funciona
  quando não há GLES utilizável (caso do virtio-gpu).
- Gate MQEMU continua PASS com esta ISO.

## O que **não** prova

- iGPU/dGPU real, femtovg/GL, frame pacing, VSync, DPMS, hotplug.
- Slint na 1ª tentativa (GL) — em QEMU ela falha e o HUD vem do fallback.
- VT real (`tty7`) com teclado/rato físicos, touch, áudio, webcam.
- M4/M5 (metal) — continuam **AWAITING_HW**.

## Known issues

- Pequeno artefacto (poucos pixels escuros) no canto superior esquerdo no
  backend `winit-software`; cosmético, suspeita de região suja do scanout.
- `arboard` avisa que o compositor não expõe `wlr-data-control`
  (clipboard Wayland); cai para X11 — inofensivo no HUD.
