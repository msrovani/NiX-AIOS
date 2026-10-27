# Evidence — Voice (0.6.0-alpha.2)

**Data:** 2026-10-27  
**Ambiente:** NixOS VM (QEMU/TCG, CPU), sem microfone/altifalante reais.  
**Modelo STT:** `ggml-tiny.bin` (fixture `fetchurl`, 75 MB, não ISO).  
**WAV:** `jfk.wav` (fixture, 344 KB, não ISO).  
**Locale:** `en_US.UTF-8`.

## Resultados

| Passo | Resultado | Tempo |
|---|---|---|
| `voice-status` | `Ready` (Whisper CLI+model; espeak TTS) | 0.33s |
| `voice-tts --no-play hello` | WAV base64 (`wav_b64`) via espeak fallback | 1.23s |
| `voice-stt --wav-path .../test-jfk.wav` | `"text"` presente (Whisper tiny) | **1405.91s** (manual) |

## Comandos

```bash
nix build .#checks.x86_64-linux.nixaios-vm -L
# no guest:
nosctl --socket /run/nosd/nosd.sock voice-status
nosctl --socket /run/nosd/nosd.sock voice-tts --no-play hello
nosctl --socket /run/nosd/nosd.sock voice-stt --wav-path /var/lib/nosd/models/test-jfk.wav
```

## Honestidade

- O `voice-stt` real **passou**, mas levou ~23 min em TCG (1 vCPU, whisper tiny,
  modelo 75 MB). Não entra no CI padrão; é manual/QEMU.
- `voice-tts` usa **espeak-ng fallback** (Piper/modelo ONNX não presente nesta VM).
- Microfone/altifalante/PipeWire real, latência e qualidade de áudio = metal.
- A fixture Whisper/WAV é de teste; **não** é seed da ISO (S1.2).
