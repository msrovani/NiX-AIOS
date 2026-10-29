# V8 — doações aderentes (oh-my-hermes + eco-tokens) — 0.6.0-alpha.15

**Data:** 2026-10-29 · **Âmbito:** D1 + V8.1–V8.6
([`VAMPIRE_PLAN_OPENJARVIS.md`](../VAMPIRE_PLAN_OPENJARVIS.md) §12).
Nada de Python/Node; zero dependências novas de runtime.

## 1. Veredicto honesto (ADR-0113)

| ID | Entrega | claim_status | Prova |
|----|---------|--------------|-------|
| **D1** | allowlist absoluta do `nvidia-smi` + inventário de spawns | **OBSERVED (host)** | `energy` → `source=nvml scope=gpu`; 5 testes em `nos-core::proc` |
| **V8.1** | `nosmcp` (stdio MCP read-only) + `nos_mcp::stdio` | **OBSERVED** | smoke-nosmcp PASS; 3 testes de transporte |
| **V8.2** | `nos-core::reported_rate` + `coverage` em `observe.energy` | **OBSERVED + unit** | `percent=100.0`, `basis=observed_cases`, `denominator=1` |
| **V8.3** | `nos-core::trim_diagnostic` + uso em erros de chat/doctor | **OBSERVED + unit** | 6 testes (ANSI, head/tail, UTF-8) |
| **V8.4** | `doctor` com `reported` vs `verified` + `unmeasured` | **OBSERVED** | `health ok reported` / `firstboot unmeasured`; `reported=6 verified=0 unmeasured=1` |
| **V8.5** | ordem de prompt cache-friendly (contrato + testes) | **VERIFIED (unit)** · medição prompt-eval **HYPOTHESIS** | `fixed_prefix_is_stable_and_user_is_last` |
| **V8.6** | loop-breaker por fingerprint (BLAKE3) em `self.heal` | **IMPLEMENTED + unit** · e2e AWAITING | 5 testes `LoopGuard`; wiring no handler |

## 2. Comandos

```bash
cd packages/nosd
cargo test --workspace        # 63+55+66… (341 → 363 testes, 0 failed)
cargo clippy --workspace --all-targets --all-features -- -D warnings   # 0
cargo fmt --all               # 0
cargo check -p nos-jarbas --features slint-ui                          # 0

NOSD_TCP=… bash scripts/smoke-nosmcp.sh    # PASS
NOSD_TCP=… bash scripts/smoke-vampire.sh   # PASS
```

**Não executado:** `nix build`/VM/QEMU (host Windows; passo 6 do plano).

## 3. Decisões de honestidade

- **`reported_rate` só em contagens**, não em dinheiro: o custo local↔nuvem é
  um contrafactual (absoluto, datado, `HYPOTHESIS`); transformá-lo numa
  percentagem seria uma categoria errada. A rate aplica-se à **cobertura**
  (turnos com fonte de energia / turnos totais).
- `coverage.percent` é `null` antes do primeiro turno (e o diagrama mostra
  `não medido`), nunca `0%`.
- `doctor` distingue `reported` (o daemon respondeu) de `verified` (nós
  verificámos localmente); `unmeasured` nunca conta como `ok`.
- **Loop-breaker não reverte nada**: recusa com `loop_detected` e
  `reverted:false`. O `git checkout .` do eco-tokens fica na membrana.
- `nosmcp` não tem tool de escrita — o smoke falha se aparecer
  `write|apply|spawn|set_transmute|nix.` no `tools/list`.

## 4. Ficheiros

Novos: `nos-core/src/{proc,reported,trim,loop_guard}.rs` ·
`nos-mcp/src/stdio.rs` · `nosd/src/nosmcp.rs` · `scripts/smoke-nosmcp.sh` ·
`docs/SPAWN_INVENTORY.md` · este ficheiro.

Alterados: `nos-core/src/lib.rs` · `nos-observe/{Cargo.toml,src/energy.rs}` ·
`nos-cortex/src/capacity.rs` · `nos-hermes/src/cognitive_bridge.rs` ·
`nos-mcp/src/lib.rs` · `nosd/{Cargo.toml,src/main.rs,src/nosctl.rs}` ·
`nosd/src/rpc/{mod,chat,health,mcp,nix_wifi}.rs` · `modules/neural-os.nix` ·
`docs/API.md` · `CHANGELOG.md` · `AGENTS.md` · `README.md` · `docs/*`.

## 5. Residuais

1. **Nix** não validado neste host (passo 6 do plano).
2. MCP: `resources/*`/`prompts/*` e HTTP/SSE **NOT_DONE**.
3. `prompt-eval` (cache hit real) não medido — HYPOTHESIS.
4. Loop-breaker ligado só a `self.heal.evaluate`; factory/mesh jobs por ligar.
5. `reported_rate` usado só na cobertura de energia (e no texto do doctor via
   contagens); o `score_v0` continua sem percentagens — decisão deliberada.
6. `nvidia-smi` no metal: se o driver não expuser o binário no perfil do
   sistema, usar `services.nosd.nvidiaSmi = "${config.hardware.nvidia.package}/bin/nvidia-smi"`.
