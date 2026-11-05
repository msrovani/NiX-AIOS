# Metal HW evidence — boot 2026-11-03 ~23:46Z (bundle `NIXAIOSLOG`)

Logs copiados da FAT32 `NIXAIOSLOG` do **mesmo pendrive do boot** (SanDisk
Ultra USB 3.0, 30,7 GB, MBR, partição 2, 1 GiB, offset `8323596288` B =
sector `16257024`). Lidos **read-only sobre o dispositivo cru** — sem montar,
sem escrever no pendrive (o Windows não atribuía letra à partição).

**Excluído de propósito**: `wifi.conf` e `firstboot.conf` (PSK em texto) e
`a4-mic.wav` (captura de voz do operador — fica só no pendrive). Nada aqui
tem credenciais.

`boots/` tem dois carimbos do mesmo boot:
`20261103T234621Z_nixaios` (M3b-audio probe) e `20261103T234622Z_nixaios`
(journal/firstboot/cage). `LATEST` → o segundo. O export repete na raiz os
ficheiros que o pen também tem na raiz.

## Hardware (de `hw-info.log`)

Dell, Intel Raptor Lake-P iGPU (`i915`), **NVIDIA RTX 3050 6GB Laptop**
(`nvidia`), MediaTek MT7925, SanDisk Ultra USB 3.0, kernel 6.18.57
(NixOS `26.11.20261020.02c087f`). Não é o Core 7 240H.

## Tested (OBSERVED neste boot)

- **Firstboot interactivo no tty1**: pt_BR, teclado `br`, `America/Sao_Paulo`,
  Wi-Fi **ROVANI** ligado; respostas persistidas no pendrive
  (`firstboot.conf`, `wifi.conf`).
- **Hotswap**: `nosd health OK` → `jarbas-cage.service` **active**, VT7, às
  23:47:17 local.
- **Transmute recusou por desenho**: `blob_deliberate: await JARBAS HITL
  disposition=Confirm` — nenhum blob de modelo/voz foi buscado em runtime
  (regra "sem seed/REJECT" honrada). HUD subiu na mesma.
- **S2 engine**: `llama-server-http` carregou `Qwen3.5-4B-Q6_K.gguf`; health
  com `PORTA 8082 · a responder`, `NGL 5`, `GPU · 2089/6144M`, `SOUL ok`,
  `landlock Verified`, `hermes running` (ticks 49→170 no follow).
- **Mesh**: `peers=0`, `mesh_seeds=[]`, `node id 15864`, `role Worker`,
  `fleet_machines=2` / `verdict unknown`. Wi-Fi ligado mas sem seeds.
- **M3b-audio gate**: `fails=0`, `gate_rc=0`, `result=SKIP`,
  `detail=aplay_card,tts_unreachable,stt_nonempty,snd_denied=0`.
  - **A1**: cards de playback presentes — `card 0 NVidia` (HDMI 3/7/8/9),
    `card 1 PCH ALC3204 Analog` com `Subdevices: 0/1` (ocupado).
  - **A4**: `arecord` 2 s → `mic_ok bytes=32044` (8 kHz mono s16). O
    microfone funciona como filho do nosd sob `DevicePolicy=closed`
    (`DeviceAllow=char-alsa rw` confirmado no `probe.log`).
  - **A6**: `play_ok=absent`.
- **ORB**: `nosd` anuncia `orb lab listening http://127.0.0.1:7701`;
  `orb.sense.audio_on=true`, `video_on=true`, `vision_ready=false`.

## Defeitos reais que este bundle expõe

| # | Sintoma (artefacto) | Leitura |
|---|---|---|
| D1 | `llama-server`: `cudaMalloc failed: out of memory` ao alocar 3352 MiB; `unable to allocate CUDA0 buffer`; `nosd-engine-s2.service: Failed with result 'exit-code'` (`journal.txt` linhas 48-57) | 1.º arranque S2 tentou `NGL 99` (health inicial) num GPU de 6 GiB com 4,1 GiB já usados → OOM. Só depois caiu para `NGL 5` e a 8082 passou a responder. O arranque não é à prova de VRAM. |
| D2 | `nvidia-smi` **SIGSEGV** (`core dumped`, `journal-boot.log:87`; `dmesg`: `segfault at 401488 … error 7 in ld-linux`) | **Causa raiz provada em 2026-11-05** (`docs/memory/SESSION_20261105_NVIDIA_SMI_SIGSEGV.md`): `0x401488 = .dynamic(0x4013a0) + 0xe8` = `DT_DEBUG.d_ptr`, que o ld.so escreve ao arrancar num `PT_LOAD` que o patchelf deixou só `PF_R`. O fix **funcionava** no store (host, glibc 2.44: original rc=139 → corrigido rc=0), mas o caminho executado aqui — `/run/current-system/sw/bin/nvidia-smi` — era o binário **por corrigir**: `buildEnv { ignoreCollisions = true }` resolve por `meta.priority` e o `nvidia_x11` tem 4 contra 5 do fix. |
| D3 | `pw-cli`/`wpctl`: `env: 'pw-cli': Arquivo ou diretório inexistente` (`a2-*.err`) | O probe mediu **PATH**, não o grafo: `/run/nosd/user` tem `pipewire-0`, `pipewire-0-manager` e `pulse/`. "PipeWire absent" é conclusão do probe, não do sistema. |
| D4 | `voice.tts` → `ok:false, "io: No such file or directory"`; `voice.turn` → `reply=EngineAbsent`, `reason=default_s1`, `play_ok=false`, transcript `"teste de audio"` | STT devolveu texto; a resposta veio do caminho keyword S1 (sem rede), não do S2; o TTS morre em ENOENT antes de sintetizar. Os symlinks `piper`, `ggml-tiny.bin`, `pt_BR-faber-medium.onnx` existem (`model-seed.log`) — o ENOENT é de outro caminho, não provado aqui. |
| D5 | `a5-voice-stt` devolve `text: ""` | `voice.stt` reporta `ok:true` com texto vazio — sucesso sem conteúdo (o mesmo género de defeito que `percent:null` combate). |
| D6 | `jarbas-cage.log`: `Unable to get gpu adapter` (Chromium), Xwayland a reiniciar; `chromium-crash/` no runtime dir | Chromium correu dentro do cage com o adaptador GPU não detectado; não há prova visual do HUD neste bundle. |

## Not-tested (honesto)

- **Nenhum screenshot/HUD**: `hotswap.log` diz cage active e o journal mostra
  `nos-jarbas` a fazer ticks de health, mas não há artefacto visual nesta
  pasta (lição 20: screendump preto não conta).
- **Core 7 240H**: este boot é a máquina RTX 3050; M4/metal da 240H continua
  AWAITING_HW.
- **A2/A6 ponta-a-ponta**: presença de grafo PipeWire e TTS audível não
  provados (D3/D4).
- **journal-follow (623 KB)**: analisado por padrões (erros/voice/engine), não
  linha a linha.
- **VERIFIED exige repetição**: isto é **1 boot** (OBSERVED), não 3.

## Como foi lido

Leitura crua da partição (BPB → FAT → cadeia de clusters), sem `mount` e sem
escrita; `ls`/`cat` dos ficheiros a partir do dispositivo. O WAV foi apagado
do export depois de copiado (privacidade); o pendrive não foi alterado.
