# Glossário

| Termo | Significado |
|-------|-------------|
| **NixAIOS** | Produto paralelo: `nosd` + ISO NixOS (ADR-0118) |
| **nosd** | Neural-OS Daemon (Tokio) |
| **nosctl** | CLI IPC contra nosd |
| **JARBAS** | Persona/UI cerimonial (Slint + cage) |
| **Hermes** | Agente OPAR Continuous |
| **Cortex** | Hub de modelos / rota / HotSwap |
| **OPAR** | Observe–Plan–Act–Verify–Remember |
| **CapGate** | Bitmask de capacidades server-authored |
| **Frugal** | wasmi 256 KiB heap / fuel 100k (path A) |
| **HotSwap** | Troca de backend GGUF em runtime |
| **Transmute** | Conversão firstboot → AIOS Full + downloads + hotswap GUI |
| **HITL** | Human-in-the-loop (confirmação JARBAS) |
| **nsgdb** | Substrato de memória (aqui: facade `seg.bin`) |
| **SoulMirror** | Snapshot frota/persona para HUD/bus |
| **CRDT thin** | Merge LWW+VV in-process (sync P2P residual) |
| **Landlock** | Confinamento FS no processo nosd (Linux) |
| **SITE_POLICY** | Auto/Confirm/Escalate/Deny para acções Nix |
| **ADR-0113** | Taxonomia honesty: OBSERVED / VERIFIED / AWAITING |
| **S1 / S2** | Inferência rápida local vs fila async mais pesada |
| **cage** | Compositor Wayland kiosk (DRM tty7) |
| **NIXAIOS051** | Volume ID da ISO release 0.5.x (workspace 0.6.0+) |
| **NIXAIOS050** | Volume ID legado da linha 0.5.0 (pré-ship Git) |
