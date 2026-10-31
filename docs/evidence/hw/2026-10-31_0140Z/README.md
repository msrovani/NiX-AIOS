# HW image — S1 0.8B (CPU) + S2 4B Q6_K (CUDA) + cinematic ORB com DIAG

Data: build 2026-10-31 (ISO `4f5ce467…`). Substitui o build `9470e357…`
(sem o fix `DeviceAllow`); a `.img` desse build foi apagada.
Base de código: `d4cacfa` (Chromium Ozone/sandbox) + `6bcc3fc`
(`char-nvidia` DeviceAllow). `nosd` reporta **0.7.0-alpha.2** (sem bump).

## Artefactos

| Ficheiro | Bytes | sha256 |
|---|---|---|
| `nixaios-hw-cuda-051-logfat.img` | 21 515 730 944 (20.0 GiB) | `f7da284319b8735f2180ad4d6e820d15853e312b6b4886dc4c3d47d233d9e2ec` |
| ISO interno | 17 220 493 312 (16.0 GiB) | `4f5ce467c43c9825d76ac25eacf20d1caecfb13919286a27b605f49109eb660b` |

Verificação: `IMG_TAIL_SHA == ISO_TAIL_SHA`
(`a0fcf3113378769e0212bfb84d002d231b7e3fdaf7e0459c0372fea430833af9`, bytes
512..fim do ISO) — só MBR/serial FAT diferem.

Closure do ISO (`/nix/store/lh17hdqc…-closure-info/store-paths`):
5 GGUFs `Qwen3.5`; `9g8k7n1z…-jarbas-cinematic-wrap` com `NIXOS_OZONE_WL`;
units `nosd` e `nosd-engine-s2` com `DeviceAllow=char-nvidia rw` (sem
`frontend`/`ctl`).

Histórico: `9470e357` (img `5eba41ec` → repack `2e032b9a`) não tinha o fix
`char-nvidia`; foi substituído por este build. Partições: ISO 16G, EFI 3M,
`NIXAIOSLOG` FAT32 4G.

**Precisa de pendrive ≥ 32 GB.**

## O que vai na imagem

- **S1** — Qwen3.5 **0.8B Q4_K_M** em **CPU** (`NOSD_LLAMA_NGL=0`,
  `NOSD_LLAMA_MODEL=…/Qwen3.5-0.8B-Q4_K_M.gguf`, `--fit on --fit-ctx 2048`).
- **S2/PRO** — Qwen3.5 **4B Q6_K** em **CUDA**: unit `nosd-engine-s2`
  (127.0.0.1:8082) com
  `--fit on --fit-ctx 2048 --reasoning off` (sem `-c`/`-ngl` fixos; o fit
  escolhe camadas+ctx que cabem na VRAM).
- **5 GGUFs** no `models/` (0.8B Q4/Q8, 4B Q4/Q6, 9B Q4) — o 9B não cabe em
  6 GB, existe só para `brain.plan/accept`.
- **DeviceAllow NVIDIA**: `char-nvidia rw` (cobre `/dev/nvidiaN` **e**
  `/dev/nvidiactl`, major 195) nos units `nosd` e `nosd-engine-s2`;
  `char-nvidia-frontend`/`char-nvidia-ctl` não são aliases válidos e foram
  removidos (check `checks.hw-cuda-deviceallow`).
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
