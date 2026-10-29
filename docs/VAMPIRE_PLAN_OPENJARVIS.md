# Plano Vampiro — absorção selectiva do OpenJarvis

**Estado:** PROPOSTA (não aceite até confirmação do dono).
**Âmbito:** copiar **ideias, specs e algoritmos** do
[`open-jarvis/OpenJarvis`](https://github.com/open-jarvis/OpenJarvis) para o
JARBAS/`nosd`. **Nunca** o runtime (Python, React, Tauri, PyO3).
**Alvos:** `packages/nosd` (0.6.x) · HUD `nos-jarbas` · ISO `NIXAIOS051`.
**Doutrina:** ADR-0113 (honesty) · ADR-0118 (portabilidade) ·
[`AGENTS.md`](../AGENTS.md) · [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md).

> *"Ultron não come aquilo que o mata."* — a lista de **veneno** (§2) é a parte
> mais importante deste plano. Tudo o resto é apetite.

---

## 0. As cinco leis da sucção

| # | Lei | Consequência prática |
|---|-----|----------------------|
| **L1** | Só se suga o que o `nosd` **não tem** | §3 é o mapa de duplicação: o que já existe é recusado por nome |
| **L2** | Entra como **PORT (Rust)** ou **SPEC (doc)** — nunca como dependência | zero Python, zero Node, zero `uv`, zero FOD na ISO |
| **L3** | Nada entra sem `required_cap` + OPAR + teste no nível do bug + evidência | `docs/evidence/VAMPIRE_<id>_<release>.md` com `claim_status` |
| **L4** | O veneno é proibido **por nome**, não por intenção | §2 fecha a porta antes de haver tentação |
| **L5** | Uma mordida = uma tranche; não bloqueia M3/M4/M5 | regra 10 do AGENTS.md ("uma release, um objectivo") |

**Regra de código:** se a peça original tiver **≤200 linhas e zero deps
pesadas**, copia-se com atribuição (`NOTICE`). Acima disso, **reescreve-se**.
Copiar 6 MiB de Python "aos pedaços" é a forma mais lenta de o portar.

---

## 1. Tipos sanguíneos (licença)

- OpenJarvis é **Apache-2.0**: derivado é permitido com `NOTICE` e indicação de
  alterações. Ideias/algoritmos não são copyrightáveis; **código é**.
- Não se copia prosa nem código do `neural-os-core` — essa regra é do
  [ADR-0118](BOOT_TO_JARBAS.md) e aplica-se por analogia: **spec > código**.
- Onde houver dúvida, a mordida entrega um **documento** no NixAIOS e um
  comentário `// inspirado em open-jarvis/OpenJarvis <path> (Apache-2.0)`.

---

## 2. Veneno — o que NUNCA atravessa a membrana

| Veneno | Origem | Regra que viola |
|--------|--------|-----------------|
| `posthog`, `posthog-js` | deps core + `frontend/` | local-first; default de telemetria ligado |
| `shell_exec`, `code_interpreter`, `docker_shell_exec`, `repl` | `tools/builtin/` | RCE no mesmo PID que detém o token do engine — fura o CapGate (regras 1/4) |
| `sandbox-wasm` (**wasmtime**) | `sandbox/wasm_runner.py` | wasmtime path A = REJECT (ADR-0059/0118) |
| `permission_memory` (`always_approve` por fingerprint) | `tools/approval_store.py` | regra 2: HITL é nonce de uso único, server-authored |
| `CapabilityPolicy(default_deny=False)` | `security/capabilities.py` | regra 4: fail-closed |
| `@tauri-apps/plugin-shell`, `plugin-updater`, `self_update_cmd` | `frontend/`, `cli/` | appliance imutável; self-update fora da store |
| `config.toml` como fonte de verdade em `$HOME` | `configs/openjarvis/` | estado≠configuração; `ProtectHome` quebra-o em silêncio |
| Engines cloud por omissão (`openai`/`anthropic`/`google`) | `engine/` | local-first; sem nuvem implícita |
| React/Tailwind/Tauri na ISO | `frontend/` | Node 22 + npm 11 no release (packaging) |
| Ollama obrigatório (`:11434`) | `scripts/quickstart.sh` | ADR-0118 hotswap-hf: "não embutir Ollama" |
| `jarvis doctor` com self-heal de estado | `scripts/install/bg-orchestrator.sh` | artefacto tem de existir **antes** da máquina |

---

## 3. Mapa de duplicação — o que **não** se morde

| Capacidade | Já temos | Fonte | Veredicto |
|---|---|---|---|
| Engines / attach | `nos-cortex::hotswap` + `NOSD_LLAMA_SERVER` | G2/T2 | não morder |
| Agentes / loop | `nos-hermes` OPAR + S2Queue | M3c | não morder (ler padrões de UI) |
| Memória | `nos-memory` (nsgdb `seg.bin`) | M8.1d | não morder |
| Skills | S1/S2 (BLAKE3 + Ed25519 + allowlist + caps) | alpha.8/9 | morder só o **formato** (V6) |
| Frota | `nos-mesh` Ed25519 + Noise + CRDT | M6/M8 | não morder |
| HITL | nonce server-authored + `nosctl consent` | S-04 | morder só a **fila/tier** (V4) |
| Tools | `nos-wasm` wasmi 256 KiB / 100k fuel | N.8 | não morder |
| Scheduler | systemd timers | — | não morder |
| Telemetria | `nos-observe` (sysfs, udev, NM) — **sem energia** | NX.3 | morder energia (V1) |

---

## 4. As mordidas

Rubrica: **GOAL · SEEK · MEASURE · OK · LOST · claim**.

### V0 — `nosctl doctor` + `config.show` + pesos métricos
**Sangue:** `cli/doctor_cmd.py` (forma), `configs/openjarvis/config.toml`
(`[learning.metrics]`).
**Destino:** `packages/nosd/crates/nosd/src/nosctl.rs` · `nos-core` (pesos).

| | |
|--|--|
| GOAL | Um ecrã de triage e uma config legível; pesos de score explícitos |
| SEEK | `nosctl doctor` · `nosctl config.show --json` |
| MEASURE | `nosctl doctor --json` → `{ok:true,sub:{health,score,engine,landlock,firstboot,mesh}}`; exit 0 só com sub verde |
| OK | doctor agrega `health`+`score`+`observe`+engine+landlock+firstboot+mesh; `config.show` nunca imprime token/segredo |
| LOST | doctor "verde" com subsistema em falta (SKILL: SKIP≠OK) |
| claim | IMPLEMENTED (host) |
| esforço | 1,5 dia · **zero deps** |

Pesos: `services.nosd.score.metrics = { accuracy, latency, cost, efficiency }`
(default 0.6/0.2/0.1/0.1, **declarado**, não escondido). Alimenta `score_v0` e o
`Cost`/`Gain` do `brain.plan`.

### V1 — Energia por turno (o buraco maior)
**Sangue:** `telemetry/{energy_nvidia,energy_rapl,energy_amd,flops,itl}.rs`
(41 KiB, self-contained).
**Destino:** `nos-observe` + `score_v0` + HUD.

| | |
|--|--|
| GOAL | joules + tokens/s + ITL do último turno, medidos ou honestamente estimados |
| SEEK | `/sys/class/powercap/intel-rapl:*/energy_uj` · `nvidia-smi --query-gpu=power.draw` |
| MEASURE | 2 turnos seguidos; contador RAPL monotónico; J/token estável ±30% |
| OK | `nosctl observe --energy` devolve `{joules, tokens, j_per_token, source:rapl\|nvml\|estimate}` |
| LOST | número sem `source`; ou estimativa apresentada como medição |
| claim | OBSERVED (host) → VERIFIED só com 3 repetições e artefacto |
| esforço | 2–3 dias · **zero deps** (sysfs + `nvidia-smi` existente) |

**Gotcha:** `powercap` exige root ou regra udev (`0440 root:powercap`). Se não
houver: `source:estimate` (CPU-time × TDP), sempre rotulado. Nunca "medido".

### V2 — Custo evitado vs nuvem
**Sangue:** `server/{savings,cost_calculator}.py` + showcase "Track Your Savings".
**Destino:** `nos-observe`/`nos-hermes` + HUD.

| | |
|--|--|
| GOAL | tornar o local-first legível: `local X € · cloud ref. Y €` |
| SEEK | `docs/PRICE_REFERENCE.md` (preços datados) × tokens do turno |
| MEASURE | `nosctl observe --cost` com `ref_date` e `ref_model` no payload |
| OK | valor sempre acompanhado de `ref_date` + `ref_model` + `HYPOTHESIS` |
| LOST | número sem fonte datada = **marketing**, proibido (ADR-0113) |
| claim | HYPOTHESIS (por definição — é contrafactual) |
| esforço | 1 dia |

### V3 — Trace OPAR visível
**Sangue:** `agents/executor.py` (display de passos).
**Destino:** `nos-hermes` (ring buffer de `CognitiveAct`) + `cognitive.trace`.

| | |
|--|--|
| GOAL | ver O→P→A→V do último turno no HUD |
| SEEK | `CognitiveAct` já gravado (`record_opar`) |
| MEASURE | `nosctl cognitive.trace --last 1` → 4 fases + ms |
| OK | fases e durações; **sem** texto de prompt/PII (redacção no servidor) |
| LOST | trace devolver conteúdo do utilizador a um canal com allowlist de browser |
| claim | IMPLEMENTED (host) |
| esforço | 2 dias |

### V4 — Fila HITL com tier e TTL
**Sangue:** `tools/approval_store.py` — **só** `pending_actions`, tiers
(`trivial/low/medium/high`) e estados (`pending/approved/denied/expired/executed`).
**Destino:** `nos-hermes::ui_window` + RPC `ui.pending`.

| | |
|--|--|
| GOAL | o utilizador vê **quantos** pedidos esperam, de que tier, com quanto TTL |
| SEEK | `ui.list` + novo `ui.pending` (`CapBits::NONE`, read-only) |
| MEASURE | 2 pedidos pendentes → HUD mostra 2 + TTL a descer; grant consome 1 |
| OK | tier+TTL na UI; grant continua nonce de uso único |
| LOST | qualquer "lembrar decisão" / `always_approve` persistido |
| claim | IMPLEMENTED (host) · VM depois |
| esforço | 2 dias |

### V5 — MCP server em Rust com CapGate (a mordida grande)
**Sangue:** `mcp/{protocol,transport,server}.py` — spec apenas (o crate Rust deles
tem 8 KiB e não serve).
**Destino:** crate novo `nos-mcp` + RPC `mcp.*` + `required_cap` por tool.

| | |
|--|--|
| GOAL | fechar o residual "VERIFIED MCP sem MCP" (`P.6 PARTIAL`) |
| SEEK | `mcp.list`/`mcp.call`; side-effect em `opar_gate` |
| MEASURE | servidor MCP de teste: `tools/list` + `tools/call` de uma tool pura; `mcp.call` com cap em falta = deny |
| OK | fail-closed por tool; sem `stdio` shell; timeouts; nenhum caminho root |
| LOST | tool MCP a chegar ao FS/NET sem passar pelo `CapGate` |
| claim | IMPLEMENTED → OBSERVED com 2 servidores |
| esforço | 1–2 semanas |

### V6 — Formato `agentskills.io` no S1/S2
**Sangue:** `skills/{parser,loader,security}.py` — **só a spec** do `SKILL.md`
(frontmatter declarativo).
**Destino:** parser em `nosd::rpc::skills` → manifesto `skill/<n>/meta`.

| | |
|--|--|
| GOAL | aceitar skills públicas sem aceitar código |
| SEEK | `skill.install --from SKILL.md` |
| MEASURE | skill com `caps: [code:execute]` → recusa; com `caps: [log]` → aceita |
| OK | mapeamento frontmatter→manifesto + BLAKE3 + publisher (S2) intactos |
| LOST | qualquer skill que peça execução sem wasm assinado |
| claim | IMPLEMENTED (host) |
| esforço | 1 semana |

### V7 — `nosctl dash` (TUI para serial/metal)
**Sangue:** `server/dashboard.py` (Textual) — forma apenas.
**Destino:** subcomando `nosctl dash` (ratatui, **feature-gated**, bin só).

| | |
|--|--|
| GOAL | HUD legível por serial quando o `screendump` sai preto com o cage |
| SEEK | `nosctl dash --serial` (sem truecolor, ASCII) |
| MEASURE | no QEMU: `console=ttyS0` mostra health+score+tick sem artefactos |
| OK | refresh 1 s, <2% CPU, zero escrita no daemon |
| LOST | TUI que exige tty gráfico ou colore com ANSI que a serial come |
| claim | OBSERVED (QEMU) |
| esforço | 1–2 dias |

### Caixa de gelo (não agora)
Streaming token-a-token no chat (SSE sobre o bridge, sem abrir métodos) ·
Markdown subset no Slint · presets como `profile` Nix · `rig-core` como backend
alternativo ao `llama-server` · compressão de sessão.

---

## 5. Sequência e gates

| Tranche | Mordidas | Pré-requisito | Gate |
|---|---|---|---|
| **T-A "sangue seco"** | V0 | nenhum | `cargo test --workspace` + `nosctl doctor` manual |
| **T-B "metabolismo"** | V1 + V2 | T-A | `journalctl -u nosd` com `source=rapl` ou `estimate` |
| **T-C "espelho"** | V3 + V4 | T-B | HUD em QEMU; `ui.pending` **não** na allowlist do kiosk até haver teste |
| **T-D "mão"** | V5 | M3 **fechado** | `nixaios-vm` + check `mcp-deny` |
| **T-E "boca"** | V6 | V5 (ou independente) | `skill.install` com recusa de `code:execute` |
| **T-F "olho"** | V7 | M4 metal em curso | QEMU serial + metal |

Regra de ouro: **T-A e T-B não podem bloquear M3/M4/M5**. Se colidirem,
pausam-se — o Norte é o Norte.

---

## 6. Orçamento (**HYPOTHESIS**)

| Mordida | Dias |
|---|---|
| V0 | 1,5 |
| V1 | 2–3 |
| V2 | 1 |
| V3 | 2 |
| V4 | 2 |
| V5 | 7–10 |
| V6 | 5 |
| V7 | 1–2 |
| **Total** | **~22–27 dias** (≈5–6 semanas a meio-tempo) |

Comparação: o port completo do produto seria **~170–190k linhas** (≈7–8× o
`nosd` inteiro). Isto é **~4% do custo** e entrega ~80% do valor visível.

---

## 7. Riscos e critérios de aborto

| Risco | Mitigação | Aborta se |
|---|---|---|
| Scope creep ("só mais um painel") | L5: uma mordida por tranche | 2 mordidas na mesma tranche |
| Métrica de vaidade (energia mal calibrada) | `source` obrigatório + repetição | número sem `source` |
| Abrir a allowlist do kiosk | método novo só `CapBits::NONE` + teste de contrato | precisar de escrita no browser |
| MCP virar superfície de RCE | sem `stdio`, sem shell, cap por tool | qualquer caminho que evite o `CapGate` |
| Contrafactual de custo virar marketing | `ref_date` + `HYPOTHESIS` | falta de fonte datada |
| Deriva de identidade do JARBAS | copiar informação, não estética | proposta de "tema OpenJarvis" |

Aborto global: se `bash scripts/gate-boot-to-jarbas.sh --strict` regredir por
causa de uma mordida, a mordida é revertida — não o gate.

---

## 8. Definição de DONE (por mordida)

1. `cargo fmt --check` + `clippy -D warnings` + `cargo test --workspace` verdes.
2. Teste **no nível do bug** (ex.: `ui.pending` não devolve payload de HITL;
   RAPL sem root → `estimate`).
3. `docs/evidence/VAMPIRE_<id>_<release>.md` com `claim_status` e artefacto.
4. `CHANGELOG.md` + [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md)
   actualizados (`P.6` para V5, `DB.1` se V1 tocar nsgdb).
5. Se tocar a ISO: `bash scripts/gate-boot-to-jarbas.sh --strict` verde.

---

## 9. Primeira acção (esta semana)

```bash
# 1) V0 — doctor + config.show (zero deps)
#    tocar: packages/nosd/crates/nosd/src/nosctl.rs, nos-core, modules/nosd*.nix
cargo test --workspace
nosctl doctor --json

# 2) evidência
docs/evidence/VAMPIRE_V0_<release>.md

# 3) só depois: V1 (RAPL) — medir antes de mostrar
cat /sys/class/powercap/intel-rapl:0/energy_uj
```

---

## 10. Anti-goals explícitos

Fork do OpenJarvis · runtime Python/Node na ISO · frontend React/Tauri ·
`posthog` · `shell_exec` · wasmtime · `permission_memory` · engines cloud por
omissão · Ollama obrigatório · "tema" visual do OpenJarvis · marketplace de
skills sem assinatura.

---

## 11. Estado da execução (2026-10-29)

Evidência: [`evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md`](evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md).

| ID | Entrega | claim |
|----|---------|-------|
| V0 | `nosctl doctor` + `config-show` + pesos do score | **OBSERVED (host) + unit** |
| V1 | energia RAPL/NVML/TDP + `observe.energy` + `turn` no chat | **OBSERVED** (`nvml`/`gpu`); RAPL AWAITING_HW |
| V2 | custo local vs nuvem datado | **OBSERVED** · HYPOTHESIS |
| V3 | `cognitive.trace` redigido | **OBSERVED + unit** |
| V4 | `ui.pending` (tier+TTL, sem nonce) | **OBSERVED + unit** |
| V5 | `nos-mcp` in-process + `mcp.list/call` | **OBSERVED** · stdio/SSE NOT_DONE |
| V6 | `skill.import` (`SKILL.md`) | **OBSERVED + parser unit** |
| V7 | `nosctl dash` (`--plain`) | **OBSERVED (host)** · QEMU AWAITING |

Pendente antes de release: `nix-instantiate --parse` dos módulos + `nix build`
da ISO + VM test + (V1) medição RAPL real. O HUD Slint já mostra energia/fila
HITL/domínio OPAR, mas não foi visto em QEMU/metal.

---

## 12. Próximos passos — doações aderentes (OMH + eco-tokens)

Filtro aplicado: só entra o que serve os **próximos passos reais** (V8 + Nix/QEMU).
Cada item tem origem, critério de aceite e claim esperado. Nada disto toca na
ISO nem no M3/M4/M5.

### A. Dívida aberta (fazer já)

| ID | Item | Origem | Aceite | Esforço |
|----|------|--------|--------|---------|
| **D1** | Allowlist do spawn `nvidia-smi` em `nos-observe::energy` (`NOSD_NVIDIA_SMI`, default absoluto `/run/current-system/sw/bin/nvidia-smi`, inventário de spawns) | auto-auditoria V1 + padrão `PROCESS_SPAWN_ALLOWLIST` (OMH) | teste: sem env usa o default absoluto; `PATH` não decide; inventário no doc | 30 min |

### B. V8 — tranche `0.6.0-alpha.15` (aderente, ~3 dias)

| ID | Item | Origem | Aceite | Esforço |
|----|------|--------|--------|---------|
| **V8.1** | `nosmcp`: bin **stdio** read-only (MCP) que fala IPC com o `nosd`; tools `nsd.health`/`nsd.score`/`nsd.energy`/`nsd.pending`/`nsd.trace`; **sem** escrita | transporte de `eco-tokens/templates/mcp-server.py`; fronteira do `nos-mcp` (V5) | `initialize`/`tools/list`/`tools/call`/`ping`; parse error `id:null`; nenhuma tool de escrita existe; caps do peer aplicadas; smoke | 1 dia |
| **V8.2** | `nos-core::reported_rate`: `percent:null` + `basis=no_observations` quando denominador vazio, `numerator_of`/`denominator_of`/`excluded`; `meets_target` falha se não medido. Usar no custo/poupança (V2) e no `score_v0` | `omh_reported_rate/v1` (OMH) | testes: denominador vazio ⇒ null; numerador composto nomeado; nenhuma percentagem sem denominador | 3 h |
| **V8.3** | `nos-core::trim_diagnostic`: strip ANSI ECMA-48, 5 head + 15 tail quando >20 linhas, cap 1000 chars, marcador de elisão; usar em `self_heal`/factory e no gate | `smart_trim` (eco-tokens) | testes com ANSI, >20 linhas, vazio; determinístico | 2 h |
| **V8.4** | `doctor` com `reported` vs `verified` por subsistema; `unmeasured` nunca é `ok` | vocabulário `prepared_not_observed` (OMH) | `health reported ok` ≠ `firstboot verified ok`; exit 1 só para fail | 2 h |
| **V8.5** | Ordem de prompt cache-friendly: teste de prefixo estável em `cognitive_bridge::build_chat_prompt` (FIXO→REPO→ESTADO→DINÂMICO) + medição prompt-eval | regra de cache (eco-tokens) | teste: prefixo idêntico entre turnos; medição rotulada HYPOTHESIS | 4 h |
| **V8.6** | Loop-breaker por BLAKE3 de artefacto em `self.heal`/factory: mesmo hash após falha = recusa + report | anti-loop (eco-tokens) **sem** o `git checkout` | teste: 2.ª tentativa com hash igual ⇒ `loop_detected`; tree intacto | 4 h |

### C. V9 — backlog (não agora)

| ID | Item | Origem | Porquê esperar |
|----|------|--------|----------------|
| V9.1 | DAG com isolamento de write-scope no `job.submit`/factory | OMH | precisa de quota/fila por peer (M7 residual) |
| V9.2 | Memória: `prepared` vs `approved` (candidatos fora do recall) | OMH | mexe no M8/CRDT; requer migração |
| V9.3 | Execução real de skills `agentskills.io` (hoje é manifesto) | V6 | requer modelo de activação sem wasm |
| V9.4 | Expor `observe.energy`/`ui.pending` ao kiosk | V1/V4 | exige teste de contrato na allowlist |
| V9.5 | Convenção de commit `Tested:` / `Not-tested:` | OMH | processo, não código |

### D. Rejeitado (membrana)

| Item | Origem | Motivo |
|------|--------|--------|
| `git checkout .` automático | eco-tokens | destrutivo; pode apagar trabalho por teste flaky — proibido no daemon |
| MCP sem autorização | eco-tokens | aceitável em harness local, inaceitável em serviço de sistema |
| CoD ≤5 palavras como regra global | eco-tokens | HYPOTHESIS; só com medição no brain 0.8B |
| Harness Python/TUI/`install.sh \| sh` | OMH/eco | fora da ISO; dev-only |
| Routing multi-provider cloud | OMH | contradiz local-first |

### E. Sequência

1. **D1** (30 min) → commit.
2. **V8.1** (1 dia) → smoke `nosmcp` + doc.
3. **V8.2 + V8.3** (5 h) → testes puros em `nos-core`.
4. **V8.4** (2 h) → `doctor` + smoke.
5. **V8.5 + V8.6** (1 dia) → medição + teste de loop.
6. Só então: WSL → `nix-instantiate --parse` → `build-iso` → `gate --strict` → QEMU serial.

Regra mantida: **um item = uma prova**; `IMPLEMENTED` não sobe a `OBSERVED` sem
artefacto em `docs/evidence/`.

### Estado (2026-10-29, `0.6.0-alpha.15`)

| Item | Estado |
|------|--------|
| D1 | **feito** — `nos-core::proc` + `docs/SPAWN_INVENTORY.md` + `path` (systemd/coreutils) |
| V8.1 | **feito** — `nosmcp` stdio read-only; `smoke-nosmcp.sh` PASS |
| V8.2 | **feito** — `reported_rate` + `observe.energy.coverage` |
| V8.3 | **feito** — `trim_diagnostic` (chat + doctor) |
| V8.4 | **feito** — `doctor` reported/verified/unmeasured |
| V8.5 | **feito (contrato)** — prefixo estável; prompt-eval HYPOTHESIS |
| V8.6 | **feito (handler)** — `LoopGuard` no `self.heal`; factory por ligar |

Evidência: [`evidence/V8_OMH_ECO_0.6.0-alpha.15.md`](evidence/V8_OMH_ECO_0.6.0-alpha.15.md).
Passo 6 **fechado em QEMU** (`nix build .#nosd`, ISO `NIXAIOS051` sha256
`4c9c55a6…`, `gate --stage MQEMU` fails=0). Metal continua **AWAITING_HW**.

---

## 13. Ataque V9 (2026-10-29, `0.6.0-alpha.16`)

| ID | Item | Estado |
|----|------|--------|
| **V9.1** | DAG com isolamento de write-scope | **feito** — `nos-core::dag` + `dag.plan` + `nosctl dag-plan` (11 testes; e2e diamante/conflito). Execução fica para V10 |
| **V9.4** | kiosk: energia + fila HITL | **feito** — `observe.energy`/`ui.pending` na allowlist + teste de contrato |
| **V9.5** | `Tested:`/`Not-tested:` nos commits | **feito** — lição 36 |
| V9.2 | memória `prepared` vs `approved` | desenho (§3 da evidência) — depende do M8/metal |
| V9.3 | skills declarativas executáveis | desenho (§3 da evidência) — precisa de decisão de semântica |

Evidência: [`evidence/V9_DAG_KIOSK_0.6.0-alpha.16.md`](evidence/V9_DAG_KIOSK_0.6.0-alpha.16.md).

**V10 (proposta):** ligar o DAG ao `job.submit`/factory (execução com um nó
verificador separado do autor) e fechar o LoobGuard no factory.

