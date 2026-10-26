# Addendum ADR-0118 — Orb cinemático + Vision MoE (conector)

## Decisão

Unir **contrato VoiceOrbs** (estados + params baratos) com **lab iron-style** (vídeo + som + troca de padrões) sem fundir renderers.

| Camada | Onde | Custo |
|--------|------|-------|
| Contrato | `nos_core::orb` (`OrbState` / `OrbPattern` / `OrbSense`) | — |
| Orquestração | `nos_hermes::OrbBroker` + IPC `orb.get\|set\|sense` | baixo |
| Lab pixels | HTTP `NOSD_ORB_LAB` (default `127.0.0.1:7701`) + `static/orb_lab.html` | browser GPU |
| HUD | Slint poll `orb.get` + botão “Lab vídeo+som” | baixo |
| Vision MoE | `route` → `ModelSlot::Vision`; frame_b64 → `ORB_VISION_FRAME` | **AWAITING** blob |

## Uso

```bash
# nosd sobe lab por default
cargo run -p nosd -- --tcp 127.0.0.1:7700
# browser: http://127.0.0.1:7701/  → Ligar cam+mic
nosctl --tcp 127.0.0.1:7700 orb-get
nosctl --tcp 127.0.0.1:7700 orb-set --state listening --pattern helix
```

## Honesty

- Lab ≠ cage DRM VERIFIED.
- MediaPipe / iron Three.js completo = residual (lab actual = canvas 2D VoiceOrbs + patterns).
- Infer Vision = `vision_ready=false` até GGUF/siglip no hub; keywords já roteiam slot Vision.
