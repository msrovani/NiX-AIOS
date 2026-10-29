# Evidence — Qwen3.5-0.8B no arranque (viabilidade + iso-hw)

**Data:** 2026-10-28
**Estado:** modelo **carrega e gera** com o llama.cpp da árvore;
ligado ao first boot da `iso-hw`. Ainda **não observado em metal nem no
first boot de uma VM**.

## O repositório que o utilizador indicou

`Qwen/Qwen3.5-0.8B` (HF) é **safetensors** (`Qwen3_5ForConditionalGeneration`,
VLM image-text-to-text, 0.87B, Apache-2.0) — **não** é GGUF e não serve
directamente ao `llama-server`.

## O que serve

`unsloth/Qwen3.5-0.8B-GGUF` (GGUF, arquitectura `qwen35`, Apache-2.0):

- ficheiro usado: `Qwen3.5-0.8B-Q4_K_M.gguf` — **508 MB**;
- alternativas: Q3_K_M/Q5_K_M/Q8_0/UD-*; `mmproj-*.gguf` para visão
  (não ligado ainda);
- contexto máximo do modelo: 262144.

## Prova de compatibilidade (host, mesmo llama.cpp)

- `llama-cpp 0.6.0` desta árvore (`/nix/store/wll87…-llama-cpp-0.6.0`):
  `llama-completion -m Qwen3.5-0.8B-Q4_K_M.gguf` **carregou em ~0,24 s** e
  gerou texto (CPU, 8 threads).
- `llama-server` (o que o `nosd` usa) com `--api-key`/`--no-webui`:
  `/health` = ok; `/completion` devolveu `content`.
- **Limite observado**: `/completion` **não aplica o chat template**; com o
  Qwen3.5 (thinking) o texto degenera (“types of types…”). O engine usa
  `/completion`; chat polido exige `/v1/chat/completions` (template do GGUF)
  ou prompt formatado — residual.
- `/v1/chat/completions` no host não foi concluído (servidor ficou em
  “Loading model” com o build a competir e o modelo em `/mnt/c`, 9p lento).

## Ligação ao first boot (`iso-hw`)

- A ISO de release **não inclui** `llama-cpp` (por isso `engine=cpu-echo`);
  a `iso-hw` inclui `pkgs.llama-cpp` e acrescenta-o ao `path` do `nosd`.
- `NOSD_BOOT_MODEL_REPO=unsloth/Qwen3.5-0.8B-GGUF`,
  `NOSD_BOOT_MODEL_FILE=Qwen3.5-0.8B-Q4_K_M.gguf`,
  `NOSD_BOOT_MODEL_LOCAL=active-qwen3.5-0.8b-q4_k_m.gguf`,
  `NOSD_BOOT_MODEL_ONLY=1` (sem coder), `NOSD_LLAMA_CTX=4096`.
- `environment.variables.NOSD_FIRSTBOOT_DOWNLOAD=1` → o `transmute` (HITL)
  baixa no first boot para `/var/lib/nosd/models` e faz `hot_load`.
- Código: `transmute.rs` ganhou `NOSD_BOOT_MODEL_*` validado (repo org/model,
  ficheiro `.gguf`, nome local sem traversal) + testes.
- Precisa de **rede no first boot** (cabo; Wi-Fi exige
  `NOSD_FIRSTBOOT_ASK=1`). Sem rede: `--no-download` e fica `cpu-echo`.

## Limites

- First boot em hardware **não testado**; o download (508 MB) e o
  `hot_load` correm no `transmute` com grant HITL.
- **Chat**: `/completion` sem template (acima); visão/mmproj não ligada.
- O repo canónico safetensors exigiria conversão offline, não first boot.
- Licença Apache-2.0; o GGUF **não** entra na ISO (runtime fetch).
