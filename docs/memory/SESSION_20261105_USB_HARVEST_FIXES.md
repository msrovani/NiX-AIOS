# Sessão 2026-11-05 — colheita do pendrive, correcções do metal, memória

Memória operacional da sessão que **encontrou** a colheita metal de 3 Nov (que
estava dada como AWAITING), a transformou em evidência versionada e corrigiu os
defeitos que ela expôs. Commits: `e01e589` (evidência), `3d44cd6` (correcções +
extractor), este (lições/memória).

## Como a colheita apareceu

O bundle estava na FAT32 `NIXAIOSLOG` do pendrive desde o boot de
2026-11-03 23:46Z, mas o Windows não lhe dava letra (lição 45) e por isso o
ledger dizia `metal AWAITING_HW`. Ler `docs/evidence` ou o `git log` não
mostrava nada — o artefacto estava **fora do repo**, no dispositivo.

Diagnóstico em cadeia (o que funcionou, para repetir):

1. `Get-Disk | Where BusType -eq 'USB'` → disco 1, MBR.
2. `Get-Partition -DiskNumber 1` → p2 `FAT32 XINT13`, offset `8323596288`
   (sector `16257024`), 1 GiB — o layout do `pack-hw-usb.sh`.
3. `Get-Volume` **não listava** o volume: partição sem letra e sem montagem.
4. `[System.IO.File]::Open('\\.\PhysicalDrive1')` → `PermissionError`:
   dispositivo cru exige admin.
5. Com elevação: parser FAT32 próprio (BPB → FAT → clusters) sobre o
   dispositivo, **read-only**, sem `mount` e sem escrever.
6. Bundle copiado: 41 ficheiros / 1,6 MB → `docs/evidence/hw/2026-11-03_2346Z/`.

O script ficou versionado em `scripts/extract-usb-logs.py` (auto-detecta o USB,
auto-eleva, filtra segredos/WAV, escreve `MANIFEST.txt` com sha256, destino
`logs/` gitignored). Não voltar a montar a partição à mão nem a usar offsets
`mtools` (lição 50).

## O que o bundle provou (OBSERVED, 1 boot, RTX 3050 + RPL-P)

- firstboot interactivo no tty1 (pt_BR/br/São Paulo, Wi-Fi ROVANI) → hotswap
  → `jarbas-cage` active em VT7 às 23:47:17.
- `transmute` recusou blobs (`blob_deliberate … HITL disposition=Confirm`) e o
  HUD subiu na mesma — a regra "sem seed de blobs" a funcionar.
- S2 carregou `Qwen3.5-4B-Q6_K.gguf`; 8082 a responder; `landlock Verified`;
  `hermes running`; mesh `peers=0`, `mesh_seeds=[]`.
- M3b-audio: A1 cards ✓, A4 `mic_ok bytes=32044` (arecord como filho do nosd,
  `DeviceAllow=char-alsa`), A2 "absent", A6 `play_ok=absent`.

## Defeitos e correcções (`3d44cd6`)

| Defeito observado | Causa | Correcção |
|---|---|---|
| S2 morreu no 1.º arranque (`cudaMalloc OOM`, 3352 MiB) e recuperou a NGL 5 | `-ngl 99` fixo **anula** `--fit on` | `nGpuLayers` default 0 + `hw-cuda` sem `NOSD_LLAMA_NGL=99` |
| `voice.tts` → `io: No such file or directory` | `tts_temp_path()` cria o WAV antes do sintetizador ⇒ `is_file()` sempre verdade ⇒ piper falhado deixava 0 B "bom" e o fallback espeak nunca disparava | `wav_ready()` (>44 B) nas três decisões + erro com caminho |
| `voice.stt` → `ok:true, text:""` | whisper sem fala devolve `Ok("")` | `transcript_or_error`: vazio = erro explícito |
| A2 `pipewire_absent` com socket presente | o probe media o **PATH** (`wpctl`), não o serviço | A2 mede `$XDG_NOSD/pipewire-0`; ferramentas reportadas à parte |
| A4 gravava 8000 Hz | default do ALSA | `arecord -q -f S16_LE -r 16000 -c 1 -d 2` (formato do whisper) |

Não corrigidos (derrotas registadas em `docs/BOOT_TO_JARBAS.md`):
`nvidia-smi` SIGSEGV mesmo com `nvidia-bin-elf-pf-w`; cinematic sem adapter
GPU no Chromium; `peers=0` com Wi-Fi ligado. O 240H continua AWAITING_HW.

## Verificação desta sessão

`cargo test --workspace` rc=0; `cargo test -p nos-voice` 12 passed; clippy
`-p nos-voice -p nosd --all-targets -D warnings` rc=0; `cargo fmt --check` rc=0;
`nix build .#checks.x86_64-linux.{engine-fit-parity,nosd-voice-deviceallow}`
rc=0; `nix-instantiate --parse` nos `.nix` tocados rc=0; `bash -n` no harvest;
extractor provado com `--mounted` (filtro + manifest).

**Not-tested:** nenhuma destas correcções voltou ao metal; `--list` do extractor
contra o pendrive físico não repetido; A5/A6 continuam sem hardware.

## Lições novas (`AGENTS.md` 53–57)

53. Pendrive sem letra: ler o dispositivo cru, com parser FAT32 próprio.
54. `is_file()` não valida conteúdo — tempfile pré-criado engana fallbacks.
55. `ok:true` com conteúdo vazio é o `0%` sem denominador da voz.
56. Flag explícita (`-ngl`) anula heurística (`--fit`) — e o restart esconde.
57. Probe que mede PATH não mede o serviço; taxa default do ALSA não é a do
    consumidor.
