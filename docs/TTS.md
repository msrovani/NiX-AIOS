# TTS de alta qualidade — `audio.cpp` / Audio8 (opcional)

**Estado:** módulo **OFF por omissão**. Não entra na ISO de release nem na
`iso-hw` (tamanho + CPU). É um attach: o motor corre onde houver GPU e o
`nosd` só o invoca.

## Modelo

- [`Edge0/Audio8-TTS-Realistic-0.6b`](https://huggingface.co/Edge0/Audio8-TTS-Realistic-0.6b)
  — 601 M params (DualAR) + codec 44.1 kHz, **23 línguas (inclui pt)**, Apache-2.0.
  Requer `transformers`+`torch`+`trust_remote_code` (Python, GPU) → **não** serve
  para a ISO.
- [`audio-cpp/Audio8-TTS-Realistic-0.6b-GGUF`](https://huggingface.co/audio-cpp/Audio8-TTS-Realistic-0.6b-GGUF)
  — mesmo modelo em **GGUF** (`q8_0` 1.43 GB; `bf16` 1.89 GB), corre com
  [`audio.cpp`](https://github.com/0xShug0/audio.cpp) (`audiocpp_cli`), backends
  `cuda|vulkan|cpu`. **É este** o caminho sem Python.

## Como o `nosd` usa

`nos-voice` escolhe o backend por esta ordem:

1. `NOSD_TTS_BIN` (absoluto) + `NOSD_TTS_MODEL` → `audiocpp_cli --family audio8_tts
   --task tts --backend $NOSD_TTS_BACKEND --model … --text … --out …`;
2. `piper` (ONNX em `models/piper`);
3. `espeak-ng` (último recurso, robótico).

Binário relativo é **ignorado** (o PATH não decide — lição 35).

## Activar

```nix
services.nosdTts = {
  enable = true;
  package = pkgs.audio-cpp;              # fornecido pelo operador
  model = /var/lib/nosd/models/tts/audio8-tts-realistic-0.6b-q8_0.gguf;
  backend = "vulkan";                    # ou "cuda" / "cpu"
};
```

Na `iso-hw`, o GGUF pode ir na FAT (`NIXAIOSLOG/models/tts/…`) e o módulo aponta
para lá. **Medir** antes de prometer: 0.6B DualAR em CPU tem de gerar mais rápido
do que o áudio dura; em `nouveau` não há CUDA e o Vulkan pode não existir.

## Porque não vai na ISO

- `q8_0` = 1.43 GB (a ISO de release tem ~2.7 GB) e o motor é C++ extra.
- Sem GPU validada, o custo/benefício contra o Piper (~20 MB, tempo real em CPU)
  é negativo — **HYPOTHESIS** até haver medição no metal.
- `trust_remote_code` do repo Transformers é código Python arbitrário: nunca no
  appliance; se for usado, fora do `nosd` e com sha fixado.

## Se `audio.cpp` tiver servidor

Seria o passo seguinte: `services.nosdTts` sobe `audiocpp_server` e o
`nos-voice` fala HTTP (OpenAI-compatible `/v1/audio/speech`), como o
`NOSD_LLAMA_SERVER` faz para o chat. Evita carregar 1.4 GB por frase.
