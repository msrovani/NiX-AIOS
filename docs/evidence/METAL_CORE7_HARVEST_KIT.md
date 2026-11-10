# Kit de colheita metal — Core 7 240H (`iso-hw-cuda`)

**Objectivo:** provar **no hardware** dois critérios que até agora só existem em
QEMU/host:

1. **uma só cópia do 4B** — exactamente um processo serve
   `Qwen3.5-4B-Q6_K.gguf` (a duplicação do hub era o defeito do metal de 31 Out);
2. **argv sem `-ngl`** — nenhum token `-ngl` em nenhum motor, porque flag
   explícita anula o `--fit on` e foi a classe de `cudaMalloc: out of memory`
   (lição 56);
3. **dispositivo por motor** — cada motor que declara `CUDA_VISIBLE_DEVICES`
   tem mesmo `/dev/nvidiaN` aberto (apanha o fallback silencioso para CPU);
4. **contexto escolhido pelo `--fit`** — lido do `/props` de cada motor, não do
   log (`n_ctx` real, sem `-c` fixo);
5. **voz: uma só cópia e argv de piper/whisper** — medido no spawn real de
   `voice.tts`/`voice.stt` (W1–W4), não num `ps` parado.

Honesty ADR-0113: isto prepara e julga a prova; **não** promove nada a VERIFIED.
O bundle diz o que mediu, o doc diz o que ainda falta.

## Artefacto construído (host, 2026-11-08)

`nix build .#iso-hw-cuda` a partir de `main` (`8298694`), out-link
`~/nixaios-out/result-iso-hw-cuda`:

| Artefacto | Bytes | sha256 |
|---|---|---|
| ISO | `8 323 596 288` | `df1000aa618f966bf6cb1f11cbb6cf33b518c728d51d4f52912b571d949e90d9` |
| `.img` (um pendrive) | `9 397 338 112` | `ff10bae0dfe680f866d35515824e4a17c5992fe5d43f085cc737fa000ae50ebd` |

Repack (o `mkfs.vfat` não está no PATH do WSL):

```bash
nix shell nixpkgs#dosfstools nixpkgs#util-linux --command \
  bash -c 'FAT_MB=1024 bash scripts/pack-hw-usb.sh "$ISO" ~/nixaios-hw-cuda-logfat.img'
```

Layout do `.img` (`sfdisk -l`): `1` Empty 0→16 257 023 (7,8 G, a ISO híbrida),
`2` EFI 3 M, `3` W95 FAT32 (LBA) 1 G em `NIXAIOSLOG`.

**Integridade do embed:** `cmp -n 8322547712 -i 1048576:1048576 img iso` →
idêntico; a MBR difere no **byte 480** (entrada de partição que o `sfdisk`
reescreve). `tail -c $ISO_SIZE img | sha256sum` **não** é prova válida — o ISO
está no offset 0 e o que muda é o cabeçalho (lição 68).

**Conteúdo do squashfs** (`xorriso -extract /nix-store.squashfs` +
`unsquashfs -l`, 363 708 linhas):

```
1  Qwen3.5-0.8B-Weight-Space-Programming-Q4_K_M.gguf   (S1)
1  Qwen3.5-4B-Q6_K.gguf                                (S2 / PRO)
3  nosd-engine-s2.service
11 llama-server
506 piper|whisper
```

Ou seja: a ISO que vai ao pendrive **contém** o 4B e o unit do S2 — o harvest
mede o que corre, não o que devia estar lá.

## O que o harvest mede

| Critério | Como é medido | PASS |
|---|---|---|
| **V1** cópias do 4B | `pgrep -f llama-server\|llama-cli` + `-m` de cada `/proc/<pid>/cmdline` | `4B_PROCESSES=1` |
| **V2** sem `-ngl` | contagem do **token exacto** `-ngl` (nunca substring, lição 30) em todos os argv | `NGL_TOKENS=0` |
| **V3** 4B realmente carregado | quantos processos têm o GGUF aberto em `/proc/<pid>/fd` | `GGUF_HOLDERS=1` |
| **V4** o fit decide | `--fit on` no argv do processo do 4B | `4B_FIT=on` |
| **V5** dispositivo por motor | `CUDA_VISIBLE_DEVICES` em `/proc/<pid>/environ` **vs** `/dev/nvidia*` aberto em `/proc/<pid>/fd`; `nvidia-smi --query-compute-apps` como segunda opinião | `DEVICE_MATCH=pass` (0 motores que declaram GPU sem a abrir) |
| **V6** contexto escolhido | `GET /props` (HTTP/1.0 por `bash /dev/tcp`, sem `curl`/`python`) no `--port` do próprio motor, com `Authorization: Bearer` quando o argv traz `--api-key` | `n_ctx>0`, sem token `-c`, `--fit on`, e `n_ctx ≥ --fit-ctx` |

`DISTINCT_GPUS` é **reportado, não exigido** — ver a secção seguinte, porque na
`iso-hw-cuda` os dois motores usam a **mesma** dGPU.

No `iso-hw-cuda` são **esperados dois motores** (S1 = 0.8B, S2 = 4B) — a
cópia única é do **4B**, não do `llama-server` em geral.

### S1 e S2 não usam GPUs distintas (e o kit não finge que usam)

O `images/hw-cuda.nix` põe **os dois** na dGPU 0:

- `systemd.services.nosd…Environment = [ "CUDA_VISIBLE_DEVICES=0" "NVIDIA_VISIBLE_DEVICES=0" ]`
  (o S1 é spawn do `nosd`, herda este env);
- `systemd.services.nosd-engine-s2…Environment = [ "CUDA_VISIBLE_DEVICES=0" ]`.

Logo `DISTINCT_GPUS=1` é o resultado **correcto** no Core 7; um critério "duas
GPUs distintas" seria **falso por desenho** e um teste que o exigisse falhava
sempre. O que V5 mede é a propriedade que realmente importa e que já falhou em
metal: **o motor que diz usar a GPU abriu mesmo `/dev/nvidiaN`** — se o CUDA
recusar o device (`Permission denied`/`ENODEV`, lição 41) o `llama-server` cai
para CPU em silêncio e o `n_ctx`/VRAM denunciam-no.

Máquina: Core 7 240H tem iGPU Intel + dGPU (RTX 3050 6 GB). Pôr o S1 na iGPU
exigiria `llama.cpp` com Vulkan (o pin é `llama-cpp-cuda`), por isso não é o
caso hoje — fica registado como lacuna, não como prova.

## Voz: piper e whisper (W1–W4)

O piper e o whisper **não são serviços**: nascem e morrem dentro de
`voice.tts`/`voice.stt` (`nos-voice/src/lib.rs`), portanto um `ps` tirado de
fora não prova cópia nenhuma. O kit amostra `/proc` a cada **10 ms enquanto a
RPC corre** e lê o argv exacto de `/proc/<pid>/cmdline`.

| Critério | Como é medido | PASS |
|---|---|---|
| **W1** uma só cópia | resolução replicada de `find_piper_cli`/`find_piper_model` (1 binário, 1 `.onnx` do idioma + sidecar `.onnx.json`) e concorrência observada ≤ 1 por motor | `1/1`, `PIPER_ONNX_MATCHING=1`, `MAX_PARALLEL_*=1` |
| **W2** argv | contrato real do código: piper `--model … --output_file …` (texto no stdin); whisper `-m … -l … -f … --no-timestamps` | `PIPER_ARGV=pass`, `WHISPER_ARGV=pass` |
| **W3** TTS corre | `voice.tts --no-play` devolve `bytes` > 44 | `TTS_BYTES>44` |
| **W4** STT corre | `voice-stt --wav-path` num WAV **16 kHz** gravado com `arecord -r 16000 -c 1` | `STT_RC=0` |

Três armadilhas que o instrumento evita:

1. **`comm` mente sobre wrappers.** O `piper` da ISO é
   `<piper-standalone>/piper`, um *script* bash que faz
   `export LD_LIBRARY_PATH=…; exec "$out/piper.real" "$@"` (lição 61). O
   processo mantém o PID e passa a chamar-se `piper.real`. Por isso a
   classificação é pelo **basename do argv** (`piper`/`piper.real`), nunca por
   `comm` — e o wrapper não conta como segunda cópia.
2. **Só descendentes do `nosd` contam.** Se o ORB estiver a falar durante a
   colheita, existe um piper alheio; a cadeia de `ppid` até ao `nosd` filtra-o.
   `MAX_PARALLEL=2` passa a significar "dois motores no **mesmo** pedido".
3. **Spawn curto pode escapar à amostragem.** Se `SPAWNS_PIPER=0`, então W2/W3
   ficam `na` — nunca PASS inventado. O piper real leva centenas de ms (load do
   ONNX) e o whisper ~1 s, muito acima do intervalo de 10 ms.

Ficheiros: `voice-spawn.tsv` (`engine pid ppid argv`), `voice-mic.wav`,
`tts.{out,err,rc}`, `stt.{out,err,rc}`, e em `verdict.txt` as chaves
`PIPER_*`, `WHISPER_*`, `SPAWNS_*`, `MAX_PARALLEL_*`, `TTS_*`, `STT_*`, `W1–W4`,
`VOICE`.

### `--fit-ctx` é mínimo, não teto

Texto do binário pinado (`llama-server --version` → build 11429, commit
d812350):

```
-fitc, --fit-ctx N   minimum ctx size that can be set by --fit option, default: 4096
```

Ou seja `--fit-ctx 2048` é o **piso** que o `--fit` respeita, não o máximo —
por isso V6 exige `n_ctx ≥ fit_min`, e o valor alto (ex.: `n_ctx=262144` do
modelo nativo) não é falha. O que prova que o fit (e não uma flag) decidiu é a
**ausência** do token `-c` mais o `--fit on`.

Contexto (não é critério): `nvidia-smi` (nome/driver/VRAM), compute apps,
marcadores do journal que só o código novo emite (lição 66):

```
suggest_ngl=0 (hint only — --fit decides; set NOSD_LLAMA_NGL in the unit to override)
llama-server spawned pid=... model=... ngl=0 thermal_cap=None ...
hub slot=active conflict keep=... ignored=...
```

## 1. Construir e gravar (host)

```bash
cd /mnt/c/DEV/nix-aios
nix build .#iso-hw-cuda -o ~/nixaios-out/result-iso-hw-cuda --print-out-paths
ISO=$(readlink -f ~/nixaios-out/result-iso-hw-cuda)/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso
ls -l "$ISO"; sha256sum "$ISO"
```

Um só pendrive (ISO + FAT32 `NIXAIOSLOG` no mesmo `.img`):

```bash
FAT_MB=1024 bash scripts/pack-hw-usb.sh "$ISO" ~/nixaios-hw-cuda-logfat.img
sha256sum ~/nixaios-hw-cuda-logfat.img
```

Escrever em **DD mode** (Rufus, ou `dd of=/dev/sdX bs=4M conv=fsync`). É o disco
inteiro — apaga-o.

> Lição 44: manter o `OUT` do pack em ext4 (`~/…`), nunca em `/mnt/c` com pouco
> espaço; o `dd` WORK→OUT no mesmo FS duplica ~17 GiB.

Opcional no pendrive: `NIXAIOSLOG/models/` com o GGUF e `NIXAIOSLOG/wifi.conf`
(ver `docs/HW_TEST.md` §2).

## 2. Colher (metal)

1. Arrancar pelo pendrive, responder Q1–Q5 no tty1 (Wi-Fi/keymap/TZ).
2. Esperar o HUD ORB e o badge `S2 · llama-server-http` (não `ausente`).
3. **O script não vai dentro da ISO** (só o M3b é embutido, via
   `modules/m3b-metal-probe.nix`). Copiar `scripts/metal-core7-harvest.sh`
   para a raiz do `NIXAIOSLOG` — no Windows, depois de gravar, o pendrive
   aparece com letra; no guest a mesma FAT está em `/mnt/nixaios-log`
   (`nixaios-log-mount`).
4. Terminal (tty2 ou SSH local) como root:

```bash
sudo bash /mnt/nixaios-log/metal-core7-harvest.sh
# bundle: /mnt/nixaios-log/harvest/<UTC>/   (FAT, legível no Windows)
```

Depende só de `bash` + `coreutils` + `awk`: o `/props` é lido por
`bash /dev/tcp`, sem `curl`/`python`, que a ISO minimal não traz. Daí o bundle
cair em `/var/lib/nosd/harvest-*` quando a FAT não estiver escrevível (o script
diz `WARN` alto).

O script escreve: `units.txt`, `models.txt`, `engines.tsv`, `engine-<pid>.argv`
(um token por linha, com o valor de `--api-key` **redigido** — a chave de
loopback não viaja para o pendrive), `devices.tsv` (env vs `/dev/nvidia*` por
PID), `fit-props.tsv` (porta, HTTP, `n_ctx`, mínimo do fit, `-c`), `holders.txt`,
`nvidia-smi.txt`, `nvidia-compute-apps.txt`, `journal.txt`, `markers.txt`,
`voice-spawn.tsv`, `voice-mic.wav`, `tts.*`, `stt.*`, `verdict.txt`,
`harvest.log`.

## 3. Verificar (host, sem o metal)

```bash
bash scripts/metal-core7-harvest.sh --verify /caminho/do/bundle
```

Imprime `V1..V6` + `W1..W4` e `RESULT=PASS|FAIL` (rc 1 em FAIL; **rc 2 = N/A**
quando não há motores, i.e. `EngineAbsent`), com os PIDs infractores. Um
`VOICE=fail` dá **FAIL** mesmo com os motores LLM em `N/A` — uma falha medida
não se esconde atrás de um inaplicável. Lê o `verdict.txt` por **chave
exacta** (`s/^CHAVE=//`), nunca por substring (lição 30).

Trazer o bundle do pendrive híbrido sem letra no Windows: `scripts/extract-usb-logs.py`
(lição 53) — lê a FAT32 no dispositivo cru, por offset.

## 4. Playbook de falha

| Sintoma | Leitura | Onde olhar |
|---|---|---|
| `4B_PROCESSES=2` | duplicação do hub: dois motores com o 4B | `markers.txt` → `hub slot=… conflict keep=…`; `holders.txt` |
| `NGL_TOKENS>0` | alguém fixou `-ngl` | `engine-<pid>.argv`; `NOSD_LLAMA_NGL` no unit (`systemctl show -p Environment`) |
| `GGUF_HOLDERS=2` com `4B_PROCESSES=1` | processo órfão de um arranque anterior ainda com o ficheiro aberto | `holders.txt` + `ps -o lstart=` |
| `4B_PROCESSES=0` | o S2 não subiu | `units.txt` (nosd-engine-s2), `journal.txt` (`EngineAbsent`, `cudaMalloc`, health timeout) |
| `4B_FIT=off` | flag `-c`/`-ngl` ou `--fit` ausente no argv | `engine-<pid>.argv` |
| `DEVICE_MATCH=fail` | um motor declara `CUDA_VISIBLE_DEVICES` mas não abriu `/dev/nvidia*` (fallback silencioso para CPU) | `devices.tsv`; `journal.txt` (`Permission denied`, `ENODEV`, `unable to get gpu adapter`); `checks.hw-cuda-deviceallow` (lição 41) |
| `FIT_OK=fail` | há `-c` fixo num motor, ou `--fit on` ausente, ou `/props` não respondeu / `n_ctx` abaixo do mínimo | `fit-props.tsv` (colunas `http`, `n_ctx`, `fit_min`, `fixed_ctx`); `engine-<pid>.argv` |
| `DISTINCT_GPUS=1` | normal nesta imagem: S1 e S2 partilham a dGPU 0 | `devices.tsv` (coluna `cvd`); não é falha |
| `CTX_4B=none` | não houve motor com o 4B para ler `/props` | `engines.tsv`; ver a linha `4B_PROCESSES=0` |
| `PIPER_ONNX_MATCHING=2` | duas vozes do mesmo idioma: `find_piper_model` escolhe a 1.ª por ordem alfabética | `ls /var/lib/nosd/models/*.onnx`; C4.1 |
| `TTS_BYTES=44` | o piper correu mas não escreveu amostras (o ficheiro existe antes do escritor, lição 54) | `tts.err`; `voice-spawn.tsv` |
| `PIPER_ARGV=fail` | o spawn não trouxe `--model`/`--output_file` | `voice-spawn.tsv` (coluna `argv`) |
| `STT_RC≠0` | WAV não é 16 kHz (lição 57) ou modelo `ggml-*.bin` ausente | `stt.err`; `stat -c %s voice-mic.wav` |
| `MAX_PARALLEL_*=2` | dois motores no **mesmo** pedido (UI e chat a falar ao mesmo tempo) | `voice-spawn.tsv`: dois PIDs distintos |
| `VOICE=na` | nada medido: sem `nosctl`/motor ou spawn mais curto que a amostragem | `verdict.txt`; nunca ler como PASS |

`EngineAbsent` (sem GGUF/binário) é **honestidade**, não falha deste kit — mas
torna V1–V6 inaplicáveis e o resultado tem de ser registado como tal, nunca
como PASS (`--verify` sai com **rc 2** e imprime `RESULT=N/A`).

## 5. Registar a evidência

Bundle → `docs/evidence/hw/<YYYY-MM-DD_HHMMZ>/` (como
`docs/evidence/hw/2026-11-03_2346Z/`), e um `README.md` com:

- sha256 do ISO e do `.img` gravado, e o host (Core 7 240H, GPU);
- saída de `--verify` (PASS/FAIL/N/A por critério);
- `engines.tsv` + `holders.txt` + `devices.tsv` + `fit-props.tsv` citados
  literalmente (o `argv` já vai com a `--api-key` redigida);
- o que **não** foi provado (ex.: voz, mesh, CUDA real se `nvidia-smi` falhar).

Só depois o item passa de `AWAITING_HW` para `OBSERVED`, e só com repetição
passa a `VERIFIED` (ADR-0113).
