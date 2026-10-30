# HW image — chat corrigido + persona + DIAG (0.6.0-alpha.16)

Data: 2026-10-30 (build 12:37Z → 12:52Z)
Base: `564993f` (chat `/v1/chat/completions` + persona + hub loaded + DIAG HUD)

## Artefactos

| Ficheiro | Bytes | sha256 |
|---|---|---|
| `nixaios-hw-cuda-051.iso` | 7 369 850 880 | `5d8434fe78df83c411a3ab8698e017bc71a3d84b885fe574a9d1f87886689a16` |
| `nixaios-hw-cuda-051-logfat.img` | 11 665 408 000 | `b144bcfd501e4b4d521f2c2738d6ae7324dbe871c6c665040ad31cd651b48b57` |

Verificação: `IMG_TAIL_SHA == ISO_TAIL_SHA`
(`0163ecba…`, bytes 512..fim) — o ISO dentro da `.img` é o publicado; só o MBR
difere (`sfdisk --append`).

## O que mudou face a `0274c982`

1. **Chat**: `/completion` cru → **`/v1/chat/completions`** (system+user,
   `max_tokens=192`, `chat_template_kwargs.enable_thinking=false`); resposta
   passa por `strip_think`. Antes o metal via `JARBAS: <think>` e nada.
2. **Persona**: `build_chat_prompt` era código morto — o SOUL.md não chegava ao
   modelo. `chat`/`chat.submit` compõem persona + memória + `## User`.
3. **`hub / 0`**: o attach não marcava o slot (`ensure_loaded`) → HUD mentia.
4. **S2 falhar já não mata o chat**: cai no S1 com `warn`.
5. **DIAG no HUD**: SMI, NGL/CUDA, GPU (used/total+nome **ou** o erro do
   `nvidia-smi`), S2, porta 8082, SOUL. `nvidia-smi` com timeout 2 s + cache 20 s.
6. **`hw-info.log`**: `lspci -nnk`, `lsmod nvidia/nouveau`, `/dev/nvidia*`,
   `nvidia-smi --query-gpu`, journal de `nosd-engine-s2`.

## O que ainda NÃO está observado

- Resposta do 0.8B em CPU com persona (qualidade) e do 4B em CUDA.
- Se o `nvidia-smi` resolve no metal, se a porta 8082 responde e a VRAM usada.
- Energia (`absent/none`): a linha DIAG + `hw-info.log` dizem qual dos três
  (binário ausente, driver, sandbox) é o caso.
