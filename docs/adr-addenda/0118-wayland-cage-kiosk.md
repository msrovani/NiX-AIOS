# Addendum ADR-0118 — Wayland + cage kiosk (JARBAS)

**Status:** Accepted (revisão s510 fail-closed)  
**Data:** 2026-10-23  
**Parent:** [0118-nixaios-nosd.md](../../../docs/architecture/0118-nixaios-nosd.md)

## Decisão

A pilha gráfica do NixAIOS é **Wayland**, não X11. O compositor é um **kiosk leve** (`cage`; **Smithay** = RESEARCH).

```text
JARBAS HUD (Rust / Slint)     ← cliente Wayland
        │
     cage (~10 MB)            ← kiosk fullscreen
        │  DRM/KMS (auto-pick; pula NVIDIA 0x10de na live)
 /dev/dri/card* (iGPU)        ← display
 nosd + LLM                   ← dGPU livre
```

## Revisão s510 — stick travava no boot Wayland

**Sintoma:** USB live congelava na sequência gráfica.  
**Causas prováveis (honestidade):**

1. `defaultUnit=graphical.target` + cage a **bloquear tty1** (getty desligado) → ecrã preto se wlroots hang.  
2. `WLR_DRM_DEVICES=/dev/dri/card0` cego — em hybrid NVIDIA, card0 pode ser dGPU sem driver prop → hang.  
3. `After=systemd-udev-settle` — settle pode nunca completar em live USB.  
4. `Restart=always` sem timeout — cage zombie = “boot eterno”.

**Mitigações (IMPLEMENTED no módulo):**

| Antes | Depois |
|-------|--------|
| `graphical.target` default | **`multi-user.target`** — nosd+hub sobem sempre |
| cage @ **tty1**, getty OFF | cage @ **tty7**; **getty@tty1** ON (rescue) |
| DRM pin `card0` | **auto-pick** (skip vendor `0x10de`); pin opcional |
| udev-settle | **removido** |
| Restart=always | **on-failure** + StartLimitBurst=2 |
| PAMName=login | removido (menos deadlock seat) |
| `timeout 45s` (s510) | **removido s511** — causava loop login↔cage no stick |

## Wire canónico

| Peça | Path |
|------|------|
| Módulo | `nixaios/modules/jarbas-cage.nix` |
| ISO | `services.jarbasCage.enable` + `defaultUnit=multi-user` |
| Env | DRM auto + DejaVu/FONTCONFIG |
| Bin | `cage -s -- nos-jarbas --ui` (**sem** timeout) |
| Sidecar | `jarbas-hub` (slog journal) — prova cognitiva sem pixels |

## Honesty (ADR-0113)

| Claim | Status |
|-------|--------|
| Recipe Wayland+cage fail-closed | IMPLEMENTED (s510+s511) |
| Stick sem login-loop | AWAITING prova ISO `NIXAIOS511` |
| 60 FPS / &lt;50 MB gráfica | UNKNOWN |
| Smithay | RESEARCH |
| X11 / GNOME na ISO | **proibido** |

## Operação no stick (pós-boot)

```text
tty1  → login nixos (autologin) — journalctl -u nosd -u jarbas-cage
tty7  → cage (se DRM ok); se falhar, hub segue no journal
systemctl status nosd jarbas-hub jarbas-cage
```

## Não-objetivos

Xorg, LightDM/GDM, desktop completo, Sway com chrome de desktop.
