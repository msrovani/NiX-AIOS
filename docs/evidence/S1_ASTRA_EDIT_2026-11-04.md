# S1 swap — Qwen3.5-0.8B editado pela Astra (2026-11-04)

## O quê

O GGUF embutido na `iso-hw` (S1, brain tier 1) passa a ser o
**Qwen3.5-0.8B editado pela Astra** (Weight-Space Programming) —
commit `eba69c2`, pin completo em `flake.nix#qwen35-08b-q4km`.

## Fonte e verificação da notícia

- Post: "Sparks of Weight-Space Programming" (Rikiya T., MIT CSAIL),
  2026-11-01/02 — https://rikiyat.github.io/blog/weight-space-programming/
- Edição: sem treino/gradientes, subtractiu uma "direcção de loop"
  (thinking que nunca acaba) de down_proj das camadas 15/19 —
  131 072 pesos = 0.016% do modelo.
- Números reportados (thinking ON, 32 768 tokens):
  MATH-500 7.9→48.1% | GSM8K 13.8→67.1% | IFEval 21.0→62.3% |
  hit-token-limit 93.0→33.1%.
- Re-run independente (akhaliq, H200, protocolo do post): MATH-500 51.0%.
- Críticas publicadas (Honnibal, Goedecke): "steering"/eng. de ML
  clássica, não inteligência nova — resultado mantém-se como
  engenharia interpretability, não como salto de capacidade.

## Proveniência do produto

- Checkpoint: `RikiyaT/Qwen3.5-0.8B-weight-space-programming`
  (bit-exact vs `Qwen/Qwen3.5-0.8B` @ `2fc06364715b…`; só 2 tensores
  diferem, sha256 publicados no README; verificado por `apply_edit.py`).
- GGUF: `NotCherry/Qwen3.5-0.8B-weight-space-programming-GGUF` —
  **única** quantização existente: `Q4_K_M`, 541 903 776 B,
  LFS sha256 `0be511c8325055e1359cb642db42da34f2dc10699128fd4b64967815e3ff6a1b`
  (SRI `sha256-C+URyDJQVeE1nLZC20LaNPLcEGmRKP1LZJZ4FeP/ahs=`).
- mmproj f16 (visão) **não** é embutido — engine texto.

## Evidência do pin (host Linux/WSL, 2026-11-04)

```
nix build .#qwen35-08b-q4km        PASS → /nix/store/8wv2737hqv8wgx…
nix build .#qwen35-08b-q4km-base   PASS → /nix/store/y6qwp3ppfcbn2j… (rollback,
                                          mesmo path das evidências de metal)
sha256sum 8wv2737h…  = 0be511c8… (== LFS pin)
nix-instantiate --parse flake.nix  OK
```

Fallback preservado: `qwen35-08b-q4km-base` = unsloth Q4_K_M sem edito
(532 517 120 B, `sha256-vSWHguNff0WPis7RrcBT5ukuibxzW6O+idOKBhIdxRc=`).

## Tested / Not-tested

- **Tested:** hash-verify de ambas as derivações via `nix build`
  (o Nix só aceita os bytes do SRI); sha256sum igual ao LFS do HF.
- **Not-tested:** se o edito sobrevive à quantização Q4_K_M (o repo
  GGUF não faz claims de qualidade); chat real (thinking ON);
  ganhos com `--reasoning off`/contexto curto (provavelmente não
  aparecem); boot de `llama-server` com este GGUF em QEMU/metal.
  Quantizações Q8_0/Q6_K do checkpoint editado **não existem** — se o
  Q4_K_M mostrar degradação no metal, converter localmente a partir do
  safetensors do RikiyaT (verificado bit-exact).

## Clássico do nível do bug (ver também AGENTS.md lição 49)

O benefício é medido **em safetensors bfloat16, thinking ON, 32k ctx**.
Quantizado Q4 + `--reasoning off` + fit-ctx 2048 é um protocolo
diferente — a honestidade do pin é o hash, não o benchmark.
