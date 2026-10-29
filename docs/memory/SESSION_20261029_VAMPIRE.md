# Sessão 2026-10-29 — Plano vampiro V0–V7 (absorção selectiva do OpenJarvis)

**Contexto:** pedido para avaliar a inclusão do
[`open-jarvis/OpenJarvis`](https://github.com/open-jarvis/OpenJarvis) no
NixAIOS. Decisão: **não incluir** (runtime Python/Node, `default_deny=False`,
`shell_exec`, wasmtime path A, posthog, self-update) e **sugar** o que é
aproveitável — reimplementado em Rust atrás do `CapGate`. Plano aprovado:
[`../VAMPIRE_PLAN_OPENJARVIS.md`](../VAMPIRE_PLAN_OPENJARVIS.md).

**Versão:** 0.6.0-alpha.14 (in-tree; a última ISO publicada continua alpha.11).
**Evidência:** [`../evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md`](../evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md).

## 1. O que foi implementado (V0–V7)

| ID | Entrega | Onde |
|----|---------|------|
| V0 | `nosctl doctor`, `nosctl config-show`, pesos `NOSD_SCORE_WEIGHTS` | `nosctl.rs`, `nos-core/metrics.rs`, `rpc/health.rs`, `modules/neural-os.nix` |
| V1 | `nos-observe::energy` (RAPL/NVML/TDP + `scope`), `observe.energy`, `turn` no chat, HUD | `nos-observe/energy.rs`, `rpc/chat.rs`, `nos-jarbas` |
| V2 | `nos-core::cost` + `docs/PRICE_REFERENCE.md` | `nos-core/cost.rs` |
| V3 | `cognitive.trace` redigido + ring OPAR (64) | `nos-hermes/lib.rs`, `rpc/health.rs` |
| V4 | `ui.pending` (tier + TTL, sem nonce) | `nos-core/ui_window.rs`, `rpc/ui_orb.rs`, `rpc/mod.rs` |
| V5 | crate `nos-mcp` + `mcp.list` / `mcp.call` | `crates/nos-mcp`, `rpc/mcp.rs` |
| V6 | `skill.import` (`SKILL.md` → manifesto S1, allowlist de caps) | `rpc/skill_md.rs`, `rpc/skills.rs` |
| V7 | `nosctl dash [--plain]` | `nosctl.rs` |

**Zero** dependências novas de runtime: `nos-mcp` só usa `serde`/`thiserror`/
`nos-core`. Nada de Python, Node, uv ou FOD na ISO.

## 2. O que ficou provado (host)

```bash
cargo check --workspace --all-features                               # rc=0
cargo test --workspace                                               # 336 passed, 33 suites
cargo clippy --workspace --all-targets --all-features -D warnings    # rc=0
cargo check -p nos-jarbas --features slint-ui                         # rc=0
NOSD_TCP=127.0.0.1:7711 NOSD_IPC_TOKEN_FILE=... bash scripts/smoke-vampire.sh  # PASS
```

Ponta-a-ponta real: `chat` devolveu `turn{ joules=4.92, source=nvml,
scope=gpu, local_eur=3.4e-7, cloud_usd=1.02e-5 }`; `skill-import` recusou
`code:execute`; `mcp-call nosd.health` ok; `ui.pending` sem `confirm_id`.

## 3. Lições (também no AGENTS.md)

1. **A superfície da CLI é código de runtime.** `nosctl --help` morria com
   stack overflow (`0xC00000FD`): o `clap` constrói a árvore na stack e o
   Windows dá 1 MiB em debug. Nenhum unit test apanhava. Fix: `main()` corre o
   runtime numa thread de 16 MiB. **Smoke tem de chamar `--help`.**
2. **Triage por substring mente.** O `doctor` procurava `absent` no JSON e
   marcava 5 subsistemas como `EngineAbsent` — `llama_server:"absent"` é
   posture normal com `cpu-echo`. Warn tem de ser por campo/subsistema.
3. **Energia sem `scope` mente.** RAPL mede o **package da CPU**, NVML mede a
   **GPU**; nenhum mede a máquina. O custo local é **limite inferior** excepto
   `tdp_estimate` (`machine_estimate`). `absent` = 0 J, nunca estimativa.
4. **Superfície dinâmica (MCP)**: piso IPC `NONE` + caps **da tool** no
   handler é o padrão seguro (o registry é server-authored); não abrir a
   allowlist do kiosk sem teste de contrato.

## 4. Residuais (honestidade)

- `nix-instantiate --parse` + `nix build` + VM/QEMU **não correram** (host
  Windows sem `nix`).
- RAPL real **AWAITING_HW**; HUD não visto em QEMU (screendump preto com cage).
- MCP sem `stdio`/SSE; `skill.import` é manifesto, não runtime.
- `tokens` = `char_heuristic` (o motor devolve texto, não `usage`).
- `http_lab::allowed_methods` **intocado**: energia/trace/pending ainda não vão
  ao browser.

## 5. Próximo

1. WSL: `nix-instantiate --parse modules/neural-os.nix`, `build-iso.sh`,
   `gate-boot-to-jarbas.sh --strict`.
2. QEMU: `nosctl dash --plain` na serial; confirmar `source=rapl` (udev
   `0440 root:powercap`).
3. Expor `observe.energy`/`ui.pending` ao kiosk **só** com teste de contrato.
