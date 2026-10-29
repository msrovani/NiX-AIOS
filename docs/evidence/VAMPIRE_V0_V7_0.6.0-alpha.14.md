# Plano vampiro V0–V7 — evidência de implementação

**Data:** 2026-10-29 · **Workspace:** 0.6.0-alpha.14 (in-tree)
**Âmbito:** absorção selectiva do OpenJarvis (ver
[`VAMPIRE_PLAN_OPENJARVIS.md`](../VAMPIRE_PLAN_OPENJARVIS.md)). Zero Python,
zero Node, zero dependências novas de runtime (excepto o crate interno
`nos-mcp`, que só usa `serde`/`thiserror`/`nos-core`).

## 1. Veredicto honesto (ADR-0113)

| ID | Entrega | claim_status | Prova |
|----|---------|--------------|-------|
| **V0** | `nosctl doctor` + `config-show` + pesos do score | **OBSERVED (host) + unit** | 4 testes; smoke §6 |
| **V1** | energia por janela (RAPL/NVML/TDP) + `observe.energy` + `turn` | **OBSERVED (host, `nvml`/`gpu`)** · RAPL **AWAITING_HW** | 6 testes; `joules=4.92 source=nvml scope=gpu` num turno `cpu-echo` |
| **V2** | custo local + contrafactual de nuvem datado | **OBSERVED** · por definição **HYPOTHESIS** | `local_eur=3.4e-7`, `cloud_usd=1.02e-5`, `ref_date=2026-10-29` |
| **V3** | `cognitive.trace` (OPAR redigido) + ring buffer | **OBSERVED + unit** | sentinela `SEGREDO-XYZ` não aparece no trace |
| **V4** | `ui.pending` (fila HITL com tier + TTL) | **OBSERVED + unit** | `confirm_id` nunca aparece no payload (asserção no smoke) |
| **V5** | crate `nos-mcp` + `mcp.list`/`mcp.call` | **OBSERVED (in-process)** · stdio/SSE **NOT_DONE** | 7 testes + smoke: `mcp-call nosd.health` ok, tool desconhecida falha |
| **V6** | `skill.import` (`SKILL.md` declarativo) | **OBSERVED + parser unit** | `skill-import ok.md` → `kind=declarative, active=false`; `bad.md` recusado |
| **V7** | `nosctl dash --plain` | **OBSERVED (host)** · QEMU serial **AWAITING** | 2 testes; 1 frame no smoke |

`OBSERVED (host Windows)` **não** é `VERIFIED` no QEMU nem no metal. Nada aqui
fecha M4/M5.

## 2. Comandos executados

```bash
cd packages/nosd
cargo check --workspace --all-features                              # rc=0
cargo test --workspace                                              # rc=0
cargo clippy --workspace --all-targets --all-features -- -D warnings # rc=0
cargo fmt --all                                                     # rc=0
cargo check -p nos-jarbas --features slint-ui                        # rc=0

# ponta-a-ponta (nosd TCP + nosctl), Git Bash no Windows:
NOSD_TCP=127.0.0.1:7711 NOSD_IPC_TOKEN_FILE=.../ipc.token \
  NOSCTL=.../nosctl.exe bash scripts/smoke-vampire.sh               # PASS
```

**Não executado:** `nix build`, `nix-instantiate --parse`, VM test, QEMU.
Este host não tem `nix` (Windows) → o módulo `modules/neural-os.nix` tem de
ser parseado e a ISO construída em WSL antes de qualquer claim de release.

## 3. Superfície IPC nova

| Método | Cap | Notas |
|--------|-----|-------|
| `config.show` | NONE | env `NOSD_*` filtrado por nome (TOKEN/SECRET/KEY/PASS/PUBLISHER) |
| `observe.energy` | NONE | agregado (J/W/`source`/`scope`); nenhum dado pessoal |
| `cognitive.trace` | NONE | domínio + comprimentos; **sem** notas |
| `ui.pending` | NONE | `window_id`/tier/TTL; **sem** `confirm_id` |
| `mcp.list` | NONE | filtrado pelas caps do peer |
| `mcp.call` | NONE (piso) | caps **da tool** aplicadas no handler |
| `skill.import` | FS | allowlist de caps; `kind=declarative`; não activa |

`http_lab::allowed_methods` (bridge do kiosk) **não** foi alterado: os métodos
novos ainda não estão expostos ao browser. Expor exige teste de contrato +
decisão no ADR.

## 4. Ficheiros

Novos:
`nos-core/src/{metrics,cost}.rs` · `nos-observe/src/energy.rs` ·
`nos-mcp/{Cargo.toml,src/lib.rs}` · `nosd/src/rpc/{mcp,skill_md}.rs` ·
`docs/PRICE_REFERENCE.md` · `scripts/smoke-vampire.sh` · este ficheiro.

Alterados:
`nos-core/src/{lib,ui_window}.rs` · `nos-hermes/src/{lib,opar_gate}.rs` ·
`nos-ipc/src/lib.rs` · `nosd/src/{main,nosctl}.rs` ·
`nosd/src/rpc/{mod,health,chat,ui_orb,skills}.rs` ·
`nos-jarbas/src/slint_app.rs` · `nos-jarbas/ui/hud.slint` ·
`modules/neural-os.nix` · `docs/API.md` · `CHANGELOG.md` ·
`docs/REMIGRATION_PLAN_LEDGER.md` (P.6) · `docs/VAMPIRE_PLAN_OPENJARVIS.md` ·
`packages/nosd/Cargo.{toml,lock}`.

## 5. O HUD Slint (V1/V3/V4 visíveis)

`hud.slint` ganhou `energy_line` + `hitl_pending` (topo, sem mexer nos painéis)
e o `tick_line` passa a incluir o último domínio OPAR. O poll do
`slint_app.rs` chama `observe.energy`, `ui.pending` e `cognitive.trace` no ciclo
existente (mesmo intervalo; zero threads novas).

## 6. Bug real apanhado pelo ponta-a-ponta

`nosctl --help` morria com **stack overflow** (`0xC00000FD`) depois de a
superfície de subcomandos crescer: o `clap` constrói a árvore de `Command` na
stack e a default do Windows é **1 MiB** (debug). Correcção: `main()` passa a
correr `async_main()` numa thread com `stack_size(16 MiB)` (o runtime Tokio
nasce lá dentro — sem `block_on` aninhado).

Lição: **a superfície da CLI é código de runtime**; unit tests de `render_*`
não provam que o binário arranca. Um `nosctl --help` no smoke teria apanhado
isto no primeiro minuto — o smoke agora cobre o caminho completo.

## 7. Residuais (não fingir que está pronto)

1. **Energia**: `source=nvml` mede **só a GPU** (`scope=gpu`); RAPL mede o
   **package da CPU** (`scope=cpu_package`). O custo local é um **limite
   inferior** excepto com `tdp_estimate` (`scope=machine_estimate`). RAPL real
   continua por provar (AWAITING_HW).
2. **TDP** default = `null`; ligar `services.nosd.tdpWatts` só com valor
   documentado da máquina.
3. **MCP** sem transporte `stdio`/SSE; sem `resources/*`/`prompts/*`.
4. **`skill.import`** não tem caminho de execução: é manifesto, não runtime.
5. **TUI** não testada em serial real; **HUD** não visto em QEMU (o
   `screendump` preto com cage continua a ser o bloqueio conhecido).
6. **Nix** não validado neste host (ver §2).
7. `tokens` são `char_heuristic`, não `usage` do motor (o backend devolve
   texto). Quando houver `usage`, substituir sem mudar o contrato.
