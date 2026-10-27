# NixAIOS 0.6.0-alpha.4 — Boot→JARBAS Slint em cage/DRM

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.4  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.4`  
**Base:** 0.6.0-alpha.3

## Aceite

M3 (Boot→JARBAS) passa a ter **pixels reais**, não só slog: `jarbas-cage`
arranca em DRM/KMS, monta o compositor `cage` e mostra o HUD Slint
(`nos-jarbas --ui`). Evidência com screenshot do scanout do QEMU:
[`evidence/CAGE_SLINT_0.6.0-alpha.4.md`](evidence/CAGE_SLINT_0.6.0-alpha.4.md).

## Correções

1. **XDG_RUNTIME_DIR fora de `/run/user`.** `/run/user` é um tmpfs montado
   pelo logind no primeiro login e **escondia** `/run/user/nosd` criado por
   tmpfiles/ExecStartPre. O wrap falhava com `mkdir /run/user: EACCES`.
   Agora é `/run/nosd/user` (nosd:0700), tmpfiles + `waitScript` (root).
2. **`PATH` do unit sem `systemctl`** fazia o `waitScript` concluir que o
   seatd estava inactivo e **apagar `/run/seatd.sock` vivo**; depois o
   `seatd-launch` não conseguia abrir socket. `pkgs.systemd` no `path` e
   `systemctl` absoluto no script.
3. **`StartLimitIntervalSec`/`StartLimitBurst` estavam em `serviceConfig`**
   (systemd ignora; warnings silenciosos). Movidos para o nível do unit —
   o anti-loop passou a funcionar (`start-limit-hit`).
4. **`cargoExtraArgs` foi removido no nixpkgs 26.11** e era ignorado em
   silêncio: o `nos-jarbas` da ISO saía **sem** `slint-ui` e caía em
   “built without feature slint-ui”. Trocado por `cargoBuildFlags`.
   O binário passou de 2.9 MB para ~31 MB e linka fontconfig/wayland.
5. **Panic `Cannot start a runtime from within a runtime`** (`rpc.rs:76`):
   `#[tokio::main]` + `rpc_blocking` (runtime aninhado). O `main` deixou de
   ser `#[tokio::main]`; caminhos async usam um runtime próprio e
   `rpc_blocking` reutiliza um runtime current-thread por thread.
6. **winit caía para X11/Xwayland** (glamor falhava e o HUD morria).
   `WINIT_UNIX_BACKEND=wayland` e `WAYLAND_DISPLAY` **só no filho** —
   exportá-lo no ambiente fazia o próprio `cage` arrancar em modo aninhado.
7. **Fallback software**: `LIBGL_ALWAYS_SOFTWARE=1` quebra o EGL do wlroots
   (`kms_swrast`). O retry usa `SLINT_BACKEND=winit-software` +
   `WLR_RENDERER=pixman`.
8. **HUD**: header com `y: 0` e altura fixa (antes esticava e o texto caía
   sobre o orb), orb a 0.55 do menor lado, linhas longas com `overflow:
   elide`; fontconfig com `XDG_CACHE_HOME` gravável (sem
   “No writable cache directories” a cada arranque).

## Evidência (QEMU, Ring3_Linux/qemu)

```
jarbas-cage.service  Active: active (running)
cage -s -- .../nos-jarbas --ui --socket /run/nosd/nosd.sock
[backend/drm/drm.c:980] connector Virtual-1: Modesetting with 1280x800 @ 74.994 Hz
jarbas: JARBAS UI start backend=Err(NotPresent) wayland=Ok("wayland-0")
jarbas: JARBAS Slint HUD shown — Alt+F7 / journalctl -u jarbas-cage
```

Screenshot do scanout: `docs/evidence/qemu-cage-hud-0.6.0-alpha.4.png`.

ISO alpha.4 (`nix build .#iso`): SHA-256
`6b084e2779fc3d73e9054ddd3572b797196a6dd33ca4244849cf2678d5d1af51`;
gate `MQEMU` PASS (2/2 layers, `fails=0`).

## Honestidade

- Isto é **QEMU + virtio-gpu + llvmpipe/pixman**: prova que o caminho
  DRM/KMS → cage → Wayland → Slint funciona, **não** que a iGPU/dGPU real
  renderiza (femtovg/GL no metal) nem taxas de frame.
- No QEMU a primeira tentativa (GL/femtovg) falha porque o virtio-gpu não
  expõe GLES utilizável; o HUD aparece pelo fallback software. No metal a
  primeira tentativa deve ser a usada — **por verificar**.
- `cage`/Slint em `tty7`/VT real, hotplug de monitor, DPMS, touch e áudio
  continuam AWAITING_HW.

## Residual

- M4 metal; Wi-Fi/DRM/áudio/webcam reais.
- S-08/S-09; R-03/R-04 (ring segue desarmado).
- Artefacto pequeno (~64×10 px) no canto superior esquerdo no backend
  `winit-software` — cosmético, a investigar.
