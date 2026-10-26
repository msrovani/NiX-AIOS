# Addendum — JARBAS cinematic webcam (s515)

## Fluxo de UI

```
nosd (orb lab :7701) → cage + Chromium kiosk → getUserMedia(cam+mic)
                              ↓
                     canvas orb cinemático + vídeo fundo
                              ↓
                     orb.sense / chat → Hermes / Vision frame
```

| Peça | Papel |
|------|--------|
| Webcam | Full-bleed espelhada sob o orb; frame JPEG → `ORB_VISION_FRAME` |
| Microfone | AnalyserNode → level/equalizer → `orb.sense` |
| Som | Osciladores WebAudio (transições) + PipeWire |
| iGPU | `pickDrm` prefere vendor `0x8086`; Chromium `--enable-gpu` |
| Slint | `uiMode=slint` fallback |

## VT

Autologin tty1; UI tty7 + `jarbas-show-vt` (`chvt`). Manual: **Alt+F7**.

## Honesty

Metal cam/mic/speakers = AWAITING_HW; host lab HTML VERIFIED por desenho.
