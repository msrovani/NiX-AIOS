# ADR-0118 addendum — Voz assíncrona (TTS/síntese não bloqueia o RPC)

**Estado:** proposto (2026-11-02, `0.7.0-alpha.5` in-tree). **Substitui:** nada.
**Relaciona:** F4/F5 (`forum.ndjson` ANVIL-0013..0020), `voice.turn`/`voice.tts`,
kiosk ORB, `docs/SECURITY.md` (HTTP lab), ADR-0118.

## Contexto

`voice.turn` e `voice.tts` chamam `synthesize_wav(play = true)` **dentro** do
`dispatch` síncrono. Com `play=true` a resposta só sai depois de o áudio
terminar (piper/espeak + `pw-play`/`aplay` com `status()`), 5–30 s típicos.

O `dispatch_blocking` (ANVIL-0020) tira o handler do worker async, mas **não**
muda a semântica da ligação: o cliente continua à espera da resposta e, numa
ligação keep-alive (ORB), o pedido seguinte fica atrás. O freeze de UI por TTS
não fecha com `spawn_blocking`.

Além disso, tocar áudio no daemon cria acoplamento escondido: o som depende de
`XDG_RUNTIME_DIR`/PipeWire do processo `nosd`, não do dispositivo onde a UI
está a correr (kiosk local, SSH, browser noutro host).

## Opções

| # | Desenho | Prós | Contras |
|---|---------|------|---------|
| A | UI/kiosk toca o WAV devolvido (`play=false` no daemon) | resposta rápida; áudio no dispositivo certo; sem estado no daemon; WebAudio no Chromium | UI tem de gerir autoplay/gesto e fila; clientes não-browser precisam de `aplay`/`pw-play` próprios |
| B | Fila TTS no daemon + `job_id` + `voice.status`/evento | um só dono do áudio; funciona sem browser; `play_ok` honesto quando o evento chega | máquina de estados, TTL de jobs, evento/stream novo, mais superfície de API |
| C | `spawn` + esquecer (pré-alpha.5) | não bloqueia | perde erros e `play_ok`; mentir sobre “played”; ficheiro temp apagado a meio |

## Decisão proposta

1. **Curto prazo (UI kiosk):** o ORB usa `play=false` e toca o WAV no browser
   (A). `voice.turn` responde com o WAV + `play_ok:null`/`not_requested`, nunca
   `play_ok:true` inventado. `voice.tts` mantém `play=true` como opt-in
   explícito para CLI/headless (síncrono, documentado como bloqueante).
2. **Médio prazo:** fila TTS com `job_id` e `voice.status` (B) para headless e
   para o `nosctl`; `play_ok` só é `true` depois do evento de fim.
3. Nada de `spawn`+esquecer (C): perder erros é a mentira que a alpha.5 fechou.
4. O `dispatch_blocking` mantém-se: isolamento de handlers bloqueantes é
   correto por si, mesmo com voz assíncrona.

## Critérios de aceite

- `voice.turn` com `play=false` responde em < 1 s (STT à parte) e devolve WAV
  válido; medido com `time nosctl` + log de bytes.
- ORB: dois pedidos seguidos na mesma ligação não ficam atrás do áudio.
- `play=true` continua a devolver `play_ok` real (já testado? ver evidência).
- Sem `play_ok:true` quando o pedido não tocou.
- Testes: handler com `play=false` não chama `play_wav_file`; `play=true` com
  `aplay` falso mede ok/fail.

## Fora de escopo

- Streaming de TTS por chunks (WebAudio/pipe) — outra ADR.
- Piper em GPU, vozes neurais novas, wake-word.
- Mudar `DeviceAllow`/PipeWire (já coberto por `char-alsa`/cage).

## Consequências

- O kiosk deixa de depender do áudio do daemon; SSH/headless continua com
  `play=true` síncrono até B existir.
- `voice.turn` passa a poder responder antes de o som sair — a UI tem de
  mostrar estado de áudio, não assumir que “resposta = som”.
