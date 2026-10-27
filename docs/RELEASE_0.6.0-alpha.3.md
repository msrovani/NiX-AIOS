# NixAIOS 0.6.0-alpha.3 — Hot download + voz

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.3  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.3`  
**Base:** 0.6.0-alpha.2

## S-04 — download a quente com grant

- `model.fetch`/`firmware.fetch` exigem `confirm_id` single-use, kind
  `blob.fetch`/`firmware.fetch`, emitido por `ui.action` de uma janela HITL.
- Replay do grant volta a `await JARBAS HITL`.
- `nosctl fetch --confirm-id <id>`; `nosctl ui-open ... --grant-kind blob.fetch`.
- `blob.deliberate` (planeamento) não consome grant; só o Act.

## Voz

- `nosctl voice-stt --wav-path <wav>` e `nosctl voice-tts [--no-play]`.
- VM: `voice-status Ready` (Whisper CLI+model, espeak TTS).
- `voice-tts --no-play hello` → WAV base64 em ~1.2s (espeak fallback).
- `voice-stt` com `ggml-tiny.bin` + `jfk.wav` passou, mas ~1405s em TCG
  (manual/QEMU; não CI).
- Fixtures Whisper/WAV são `fetchurl` de teste; **não** entram na ISO.

## Evidência

- [`docs/evidence/VOICE_0.6.0-alpha.2.md`](evidence/VOICE_0.6.0-alpha.2.md)
- `nixaios-vm` PASS: GGUF/chat/unload/reload + `accept-m3.sh` +
  `voice-status`/`voice-tts`.
- `ipc_auth`: grant de blob, replay, fetch sem HITL.

## Residual

- Cage/Slint/DRM, firstboot tty, Wi-Fi real.
- S-08/S-09, R-03/R-04.
- Metal M4/M5; voz/mic/PipeWire real; CUDA/ROCm.
