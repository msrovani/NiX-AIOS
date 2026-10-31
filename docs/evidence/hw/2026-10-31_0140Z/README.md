# HW image — S1 0.8B (CPU) + S2 4B Q6_K (CUDA) + cinematic ORB com DIAG

Data: build 2026-10-31 (ISO `2ee396fd…`) com a revisão de simbiose
(`92fb47a`): voz (`char-alsa` + `XDG_RUNTIME_DIR`), recall novo e paridade do
motor. Substitui `4f5ce467…` (sem voz/XDG) e `9470e357…` (sem DeviceAllow).
Base de código: `92fb47a` + `6bcc3fc`/`72069a1` (DeviceAllow) + `d4cacfa`
(Chromium Ozone). `nosd` reporta **0.7.0-alpha.2** (sem bump).

## Artefactos

| Ficheiro | Bytes | sha256 |
|---|---|---|
| `nixaios-hw-cuda-051-logfat.img` | 21 516 779 520 (20.0 GiB) | `4e339f371fa09ec0db8e54e3b204fb8222886b92b66baa9513acbb86deeba487` |
| ISO interno | 17 221 165 056 (16.0 GiB) | `2ee396fd3ff4d65559f0a9340decd218f51782da16551c17520b97fd86def614` |

Verificação: `IMG_TAIL_SHA == ISO_TAIL_SHA`
(`5db01e7280ff8b1572bb373cbc6d581dc5ef9d51607f81b7b788281e34fe8722`, bytes
512..fim do ISO) — só MBR/serial FAT diferem.

Closure do ISO (`/nix/store/9d952a73…-closure-info/store-paths`):
5 GGUFs `Qwen3.5`; `7nvd6685…-jarbas-cinematic-wrap` com `NIXOS_OZONE_WL`;
`nosd` com `char-drm`, `char-alsa`, `char-nvidia` (sem `frontend`/`ctl`),
`XDG_RUNTIME_DIR=/run/nosd/user` e `ExecStartPre` root que cria o diretório
0700; `nosd-engine-s2` com `char-nvidia`.

Histórico: `4f5ce467` (img `f7da2843`) não tinha `char-alsa`/XDG;
`9470e357` (img `5eba41ec` → repack `2e032b9a`) não tinha DeviceAllow.
Partições: ISO 16G, EFI 3M, `NIXAIOSLOG` FAT32 4G.

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
- **Voz**: `char-alsa rw` no `nosd` (arecord/aplay como filhos sob
  `DevicePolicy=closed`) e `XDG_RUNTIME_DIR=/run/nosd/user` criado 0700 por
  `ExecStartPre` root (check `nosd-voice-deviceallow`). Antes, o metal podia
  dizer "microfone avariado" por EACCES.
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
