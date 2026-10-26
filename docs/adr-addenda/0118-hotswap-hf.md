# Addendum ADR-0118 — Hot-Load / Hot-Swap + HF fetch

**Status:** Accepted (scaffold)  
**Data:** 2026-10-23  
**Parent:** ADR-0118

## Decisão

`nosd` orquestra **probe → match → fetch → unload → load** em runtime. Engines:

| Path | Formato | Engine | Status NixAIOS |
|------|---------|--------|----------------|
| A (default) | GGUF | `llama-cli` / `llama-completion` (`-ngl`) | MVP HotSwap **IMPLEMENTED** |
| B | EXL2 | TabbyAPI / ExLlamaV2 | `EngineAbsent` até flake |
| C | AWQ/GPTQ/FP8 | vLLM / Aphrodite | `EngineAbsent` até flake |
| D | safetensors | candle (Rust) | RESEARCH |

**Não** embutir Ollama/Jan como runtime obrigatório — são referências; o daemon fala GGUF via subprocess HotSwap + opcional fetch HTTPS (HF resolve URL).

## Pipeline (honesty)

```text
CapacityProber (nvidia-smi/sysfs)
    → suggest_quant / plan_n_gpu_layers
    → fetch (HTTPS HF resolve → /var/lib/nosd/models)  [CapGate NET|FS]
    → header_scan (primeiros KB)
    → HotSwapController::unload → try_attach_llama
```

Troca GGUF medida em lab: **UNKNOWN** até prova tok/s; literatura 1–4 s = HYPOTHESIS.

## IPC

- `model.suggest` — capacity → quant hint  
- `model.fetch` — `{url}` ou `{repo,file}` → path  
- `model.load` — path → hot attach llama  
- `model.unload` — volta CpuEcho  

## Não-objetivos (MVP)

NVML linkado; auto-download sem CapGate NET; claim EXL2/vLLM Ready; candle in-process.
