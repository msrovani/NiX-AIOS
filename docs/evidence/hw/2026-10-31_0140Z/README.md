# HW image — S1 0.8B (CPU) + S2 4B Q6_K (CUDA) + cinematic ORB com DIAG

Data: build 2026-10-31 (ISO `9470e357…`), repack a 2026-10-30 22:xx.
Base de código: `d5ee259` + WIP de `modules/jarbas-cage.nix` (ozone/wayland) da
outra sessão no working tree. `nosd` continua a reportar **0.7.0-alpha.2**
(o bump de versão ficou fora do snapshot) — o que interessa é o que está lá
dentro, listado abaixo.

## Artefactos

| Ficheiro | Bytes | sha256 |
|---|---|---|
| `nixaios-hw-cuda-051-logfat.img` (repack) | 21 516 779 520 (20.0 GiB) | `2e032b9abdc46c3cc4ce3328e0f1a7e6bf85f2c2dbc2ff440a3be81527103cbb` |
| ISO interno | 17 221 107 712 (16.0 GiB) | `9470e35762a1cd599e4d3cbee5fe44dbd03d259901652ac38fef5e3a3978086d` |

Verificação do repack: `IMG_TAIL_SHA == ISO_TAIL_SHA`
(`bb82a44ba0799db4910bbe8f275f9bb3f2e0cf8f128bc46c4e175b49a91b61a7`, bytes
512..fim do ISO) — o payload é o **mesmo** ISO `9470e357`; só MBR/FAT metadata
mudam (o `mkfs.vfat` gera outro serial). O `.img` original (`5eba41ec…`) foi
apagado na coordenação de duas sessões; este repack restaura o mesmo conteúdo.

O ISO foi verificado de novo: `sha256sum` = `9470e357…`, e a closure
(`/nix/store/6gnahic…-closure-info/store-paths`) contém
`g99xicyaq74yaygdj1k7blchd47qcy1m-jarbas-cinematic-wrap`, o wrap com
`NIXOS_OZONE_WL=1`/`--ozone-platform=wayland` — a prova de que o fix do
Chromium (alpha.3, commit `d4cacfa`) está dentro da imagem.

Partições: ISO 16G, EFI 3M, `NIXAIOSLOG` FAT32 4G.

**Precisa de pendrive ≥ 32 GB** (a anterior de 11.6 GB cabia em 16 GB).

## O que vai na imagem

- **S1** — Qwen3.5 **0.8B Q4_K_M** em **CPU** (`NOSD_LLAMA_NGL=0`,
  `NOSD_LLAMA_MODEL=…/Qwen3.5-0.8B-Q4_K_M.gguf`, `--fit on --fit-ctx 2048`).
- **S2/PRO** — Qwen3.5 **4B Q6_K** em **CUDA**: unit `nosd-engine-s2`
  (127.0.0.1:8082) com
  `--fit on --fit-ctx 2048 --reasoning off` (sem `-c`/`-ngl` fixos; o fit
  escolhe camadas+ctx que cabem na VRAM).
- **5 GGUFs** no `models/` (0.8B Q4/Q8, 4B Q4/Q6, 9B Q4) — o 9B não cabe em
  6 GB, existe só para `brain.plan/accept`.
- **ORB cinematic** (Chromium kiosk) com `TELEMETRIA` a mostrar sempre
  `S2 / PRO`, `GPU` (MiB usados/total ou o erro do `nvidia-smi`) e `S2 PORTA`.
  Drawer `DIAG` com as seis linhas (SMI, NGL/CUDA, GPU, S2, porta, SOUL).
- **`--reasoning off`** no `llama-server` (build 11429 tem `-rea/--reasoning`):
  o Qwen3.x responde directo em vez de gastar tokens em `<think>`.
- Chat por `/v1/chat/completions` (template do GGUF, persona `SOUL.md`,
  `strip_think` como rede), `hub_loaded` honesto, fallback S1 quando o S2 falha.

Provas no store:
`unit-nosd-engine-s2` com `…Qwen3.5-4B-Q6_K.gguf --fit on --fit-ctx 2048
--reasoning off --no-webui`; único `nosd-0.7.0-alpha.2` cujo binário contém a
string `S2 / PRO` (o embutido no ISO).

## O que olhar no metal

1. `TELEMETRIA` → `GPU  x/y MiB` (se o 4B está na GPU, y sobe ~3.5 GiB quando
   responde) e `S2 / PRO  llama-server-http`, `S2 PORTA  a responder`.
2. Chat curto → `lane_s1` (0.8B, CPU) responde texto, **sem** `<think>`.
3. Pedido profundo → `lane_s2`; `T/S` mostra a estimativa end-to-end.
4. Se `GPU · smi ERRO: …`: driver/sandbox; `hw-info.log` na FAT tem
   `lspci -nnk`, `lsmod`, `/dev/nvidia*` e o journal do `nosd-engine-s2`.

## Não testado (honesto)

- Metal: 4B na GPU (VRAM real), latência, `--reasoning off` no Qwen3.5.
- Build em QEMU (CUDA não existe em virtio-gpu).
- O WIP `jarbas-cage.nix` (ozone/wayland) foi incluído pelo snapshot do working
  tree; o kiosk Chromium no cage continua **AWAITING_HW**.
