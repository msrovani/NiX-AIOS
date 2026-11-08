# Kit de colheita metal — Core 7 240H (`iso-hw-cuda`)

**Objectivo:** provar **no hardware** dois critérios que até agora só existem em
QEMU/host:

1. **uma só cópia do 4B** — exactamente um processo serve
   `Qwen3.5-4B-Q6_K.gguf` (a duplicação do hub era o defeito do metal de 31 Out);
2. **argv sem `-ngl`** — nenhum token `-ngl` em nenhum motor, porque flag
   explícita anula o `--fit on` e foi a classe de `cudaMalloc: out of memory`
   (lição 56).

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

No `iso-hw-cuda` são **esperados dois motores** (S1 = 0.8B, S2 = 4B) — a
cópia única é do **4B**, não do `llama-server` em geral.

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
3. Terminal (tty2 ou SSH local) como root:

```bash
sudo bash scripts/metal-core7-harvest.sh
# bundle: /mnt/nixaios-log/harvest/<UTC>/   (FAT, legível no Windows)
```

O script escreve: `units.txt`, `models.txt`, `engines.tsv`, `engine-<pid>.argv`
(um token por linha), `holders.txt`, `nvidia-smi.txt`, `nvidia-compute-apps.txt`,
`journal.txt`, `markers.txt`, `verdict.txt`, `harvest.log`.

## 3. Verificar (host, sem o metal)

```bash
bash scripts/metal-core7-harvest.sh --verify /caminho/do/bundle
```

Imprime `V1..V4` e `RESULT=PASS|FAIL` (rc 1 em FAIL), com os PIDs infractores.

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

`EngineAbsent` (sem GGUF/binário) é **honestidade**, não falha deste kit — mas
torna V1 inaplicável e o resultado tem de ser registado como tal, nunca como PASS.

## 5. Registar a evidência

Bundle → `docs/evidence/hw/<YYYY-MM-DD_HHMMZ>/` (como
`docs/evidence/hw/2026-11-03_2346Z/`), e um `README.md` com:

- sha256 do ISO e do `.img` gravado, e o host (Core 7 240H, GPU);
- saída de `--verify` (PASS/FAIL por critério);
- `engines.tsv` + `holders.txt` citados literalmente;
- o que **não** foi provado (ex.: voz, mesh, CUDA real se `nvidia-smi` falhar).

Só depois o item passa de `AWAITING_HW` para `OBSERVED`, e só com repetição
passa a `VERIFIED` (ADR-0113).
