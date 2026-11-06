# Fase 1 — a VRAM deixa de ser sobre-subscrita (F1, C1.1–C1.4 + C3.2/C3.3)

**Data:** 2026-11-06 · **Plano:** [`docs/PLAN_INFERENCE_VRAM_VOZ_FABRICA.md`](../PLAN_INFERENCE_VRAM_VOZ_FABRICA.md) §Fase 1
**Estado:** HOST OBSERVED (fmt/clippy/testes). QEMU/metal **NOT-TESTED**.

## O que mudou

| Tarefa | Mudança | Ficheiro |
|---|---|---|
| C1.1 | apagados os **três** `set_var("NOSD_LLAMA_NGL")` (boot, tick, `observe.snapshot`); `suggest_ngl` fica como hint no log | `nosd/src/main.rs`, `nosd/src/rpc/health.rs` |
| C1.2 | `HermesEngine::should_hot_load(&Backend, has_s2)` — lane S2 com motor S2 não hot-loada | `nos-hermes/src/lib.rs` |
| C1.3 | `LoadOutcome{Reused,Swapped}` em vez de `bool` | `nos-cortex/src/hotswap/mod.rs` |
| C1.4 | health do spawn configurável (default 120 s) e `SwapError::Boot` — sem fallback silencioso para `llama-cli` | `nos-cortex/src/hotswap/{llama_server,backend}.rs` |
| C3.2 | `DEFAULT_N_PREDICT = 192` único para servidor e CLI | `nos-cortex/src/hotswap/{mod,llama_server,llama_cli}.rs` |
| C3.3 | `thermal_ngl_cap: AtomicU32` no processo (`u32::MAX` = sem cap), aplicado **só** a `-ngl` explícito | `nos-cortex/src/hotswap/mod.rs` |

C3.2 e C3.3 foram arrastados para esta fase de propósito: remover os `set_var`
sem um consumidor real deixaria apenas um log inerte — exactamente o defeito
que a revisão apontou.

## Porque era P0

Um `-ngl` explícito **anula** o `--fit on` do llama.cpp. No metal de 3 Nov o
4B Q6 com `-ngl 99` foi `cudaMalloc: out of memory`; o valor vinha de
`plan_n_gpu_layers` (heurística) escrito no env. O cap térmico agravava-o:
escrever no env **depois** do spawn não afecta o motor vivo e nunca subia de
volta — um pico de calor deixava `-ngl 16` colado a todos os spawns seguintes.

## Prova executada (host, WSL)

Ferramenta: `nix develop --command` (o WSL não tem `cargo`; o devShell do flake
traz `cargo 1.99.1 (5f94df478 2026-08-27)`), log em `$HOME` (lição 51).

```
cd packages/nosd
cargo fmt --all                     # sem output
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

- `FMT=0`
- `CLIPPY=0` (sem `warning:`)
- `GATE_RC=0`, **1021** testes `ok` (eram 1015) — 6 novos:

```
test hotswap::tests::thermal_cap_only_applies_to_explicit_ngl ... ok
test hotswap::tests::thermal_cap_rises_and_falls ... ok
test hotswap::tests::load_outcome_reports_substitution ... ok
test hotswap::llama_server::tests::health_timeout_is_configurable_and_bounded ... ok
test hotswap::llama_cli::tests::n_predict_default_matches_server ... ok
test load_path_tests::s2_lane_does_not_hot_load_when_s2_engine_exists ... ok
```

`grep -rn 'set_var("NOSD_LLAMA_NGL"'' packages/nosd/crates --include=*.rs | wc -l`
→ **0**.

## Not-tested (honestidade ADR-0113)

- **QEMU** (`gate-boot-to-jarbas --stage MQEMU`): argv do motor sem `-ngl`,
  log `suggest_ngl=… (hint only — --fit decides)` e **uma** cópia do 4B no S2.
  Não corrido neste host: custa um build de ISO + boot, e o commit é de código
  de arranque — fica para o gate de release.
- **Metal**: `nvidia-smi --query-compute-apps` com o servidor e
  `offloaded N/M layers` no journal do engine.
- O caminho `SwapError::Boot` (health timeout real) não foi exercitado: só o
  parser do prazo (`health_timeout_secs`) tem teste. Um stub lento é o próximo
  passo de prova.

## Risco que fica vivo

**R1** — se o binário `llama-server` perder o `--fit`, o motor passa a correr
em CPU (0 camadas) sem aviso. Detecção: `offloaded 0/N` no stderr do engine.
Mitigação documentada em `images/hw-cuda.nix:94` e `hw-cuda-4b.nix:79`:
`-ngl` explícito no unit só quando o operador quer mandar.
