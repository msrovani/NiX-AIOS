# Fases 2–7 — fit, cliente llama, voz, fábrica, wasm e energia

**Data:** 2026-11-06 · **Plano:** [`docs/PLAN_INFERENCE_VRAM_VOZ_FABRICA.md`](../PLAN_INFERENCE_VRAM_VOZ_FABRICA.md)
**Estado:** HOST OBSERVED (fmt/clippy/testes). QEMU e metal **NOT-TESTED**.

Ferramenta: `nix develop --command` no WSL (não há `cargo` no host nem no PATH do
WSL sem o devShell), `cargo 1.99.1 (5f94df478 2026-08-27)`, logs em `$HOME`.

## O que mudou, por tarefa

| Tarefa | Mudança | Ficheiro |
|---|---|---|
| C2.1 | `fit_model(need_vram, need_ram, ram, vram, allow_spill)` + `basis: vram\|spill\|cpu`; RAM não paga VRAM | `nos-cortex/src/fit_policy.rs` |
| C2.2 | `need_estimate` = ficheiro + KV (`kv_est_mb`, `source:"estimate"`) | `nos-cortex/src/hub.rs` |
| C2.3 | `hub.evict(&hotswap)` descarrega para `cpu-echo` e depois desmarca | `nos-cortex/src/hub.rs` |
| C2.4 | `vram_free_mb` = máximo por placa; `vram_free_total_mb` para relatório | `nos-cortex/src/capacity.rs` |
| C2.5 | `register_path` recusa substituir path; `candidates` + `conflict` | `nos-cortex/src/hub.rs` |
| C3.1 | `ureq::Error::Status` → `SwapError::Http{code,body}`; retry só em 400/422 | `nos-cortex/src/hotswap/{backend,llama_server}.rs` |
| C4.1 | voz Piper só do idioma (`piper_model_matches_lang`) | `nos-voice/src/lib.rs` |
| C4.2 | STT base antes de tiny + `stt_model_reason` no `voice.status` | `nos-voice/src/lib.rs` |
| C4.3 | um backend por WAV; apaga antes do espeak | `nos-voice/src/lib.rs` |
| C4.4 | devices de `aplay -l` em vez de `plughw:0,0` fixo | `nos-voice/src/lib.rs` |
| C5.1 | `nix build --no-link --print-out-paths` + `out/bin/<name>` quando o daemon responde | `nos-factory/src/scaffold.rs` |
| C5.2 | `write` recusa directório já preenchido | `nos-factory/src/scaffold.rs` |
| C5.3 | `Lang::detect` procura o token (o `parse(intent)` inteiro nunca casava) | `nos-factory/src/scaffold.rs` |
| C6.1 | memória = `min(heap, max_pages × 64 KiB)` | `nos-wasm/src/lib.rs` |
| C7.1 | RAPL soma todos os `intel-rapl:N` | `nos-observe/src/energy.rs` |

## Prova executada

```
cd packages/nosd
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

| Fase | Gate | Testes `ok` |
|---|---|---|
| F2 | `GATE_F2B_RC=0` | 1029 (eram 1021) |
| F3 | `GATE_F3B_RC=0` | 1031 (+2) |
| F4–F7 (conjunto) | `GATE_F47B_RC=0` | 1037 (+6) |

Testes novos das fases 4–7:

```
test f4_tests::piper_model_must_match_the_language ... ok
test f4_tests::aplay_list_parses_real_cards ... ok
test f5_tests::detect_finds_the_language_token_in_a_phrase ... ok
test f5_tests::write_refuses_to_overwrite_an_existing_app ... ok
test f6_tests::memory_limit_uses_the_smaller_of_heap_and_pages ... ok
test f7_tests::rapl_sums_every_package ... ok
```

Falhas apanhadas pelo gate antes do commit (registadas porque são o valor do
gate, não ruído): `path` movido em `register_path` (E0382), `let mut resp` sem
mutação (clippy), `split(|c| c == '_' || c == '-')` (clippy: `split(['_','-'])`).

## Not-tested (ADR-0113)

- **QEMU** de qualquer fase: `gate-boot-to-jarbas --stage MQEMU` não foi corrido.
  Em particular o argv do motor (C1.1), uma só cópia do 4B no S2 (C1.2) e o som
  no `aplay` (C4.4) continuam sem observação.
- **Metal**: `nvidia-smi --query-compute-apps`, `offloaded N/M`, som audível,
  GPU no cage. `AWAITING_HW`.
- **C5.1**: `nix build` como `nosd` exige `nix-daemon` + `trusted-users`; sem
  isso `nix store ping` falha e o código cai no PATH (declarado, não silencioso).
  Nenhuma app real foi construída por nix neste host.
- **C3.1**: não houve llama-server a recusar campos; o retry está provado pela
  função pura (`retry_without_template`), não por um stub HTTP 400.
- **C7.1**: wrap de RAPL continua a ser detectado sobre o **total** somado, não
  por contador; num host com dois packages que façam wrap em instantes
  diferentes isso pode perder uma janela.
- `voice.status` lista os modelos, mas não agrupa por idioma (C4.1 pedia-o);
  `stt_model_reason` sim, está.
