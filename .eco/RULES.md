# Eco-Tokens v3 — Context Engineering & Token Economy (NixAIOS)

Regras do kit [msrovani/Eco-tokens](https://github.com/msrovani/Eco-tokens) adaptadas a
este repositório. Carregadas como `instructions` em `opencode.json`.

**Hierarquia:** `AGENTS.md` manda. Em conflito, `AGENTS.md` ganha e este ficheiro
está errado — corrigir aqui, não lá.

## [FIXO] Instruções de sistema (não alterar por tarefa)

- Stack: MCP `eco-tokens` (Python stdlib) + `scripts/verify.sh` + `.eco/state.json`.
- Tools: `get_current_task` | `update_task_status` | `run_verification` | `search_ast_symbol`.
- Papéis: CHIEF (orquestra) · SPECIALIST (edita) · CHECKER (valida).
- Anti-loop: `SHA256(git diff)` idêntico após falha → `BLOCKED` **sem build** e
  **sem revert**. O breaker **recusa**, nunca faz `git checkout .` (AGENTS.md lição 34).
- Logs: `smart_trim` (5 head + 15 tail, strip ANSI) — payload curto.

## [FIXO] Chain-of-Draft (CoD)

- Pensamento interno: **máx. 5 palavras por passo**.
- Proibido filler: "Entendi", "Vou analisar", "Certamente", "Claro", "Deixa eu ver".
- Chat visível: **3–5 linhas** operacionais; sem narrativa.
- Isto não autoriza saltar a doutrina de honestidade (ADR-0113): sem artefacto e
  repetição não há `VERIFIED`, e "curto" nunca é desculpa para omitir `Not-tested:`.

## [REPO] Regras deste repositório

- Estado canônico: `.eco/state.json` (atómico, local, fora do git).
- Verify: só via MCP `run_verification` (ou `dry_run=true` se o fingerprint está
  estável). Por baixo corre `scripts/verify.sh` = `fmt --check` + `clippy -D
  warnings` + `cargo test --workspace` em `packages/nosd`.
- Saltar um passo do verify (`ECO_SKIP_CLIPPY=1` / `ECO_SKIP_TEST=1`) obriga a
  dizê-lo no relatório: verificação parcial não é PASS.
- Busca: só `search_ast_symbol` (≤25 linhas). Sem dump de árvore/ficheiro.
- Não-negociáveis de `AGENTS.md` continuam acima de tudo: identidade não
  autoriza, HITL é server-authored, fail-closed, sem caminhos root sem auth.
- Git: stages explícitos, **nunca `git add -A`** (duas sessões no mesmo tree).
  Commit leva `Tested:` / `Not-tested:` no corpo.

## [ESTADO] Tarefa ativa

- Sempre iniciar com `get_current_task`.
- Editar só se `status` ∈ {TODO, WORKING, READY_FOR_REVIEW}.
- Se `BLOCKED`: parar. Não patchar.

## [DINÂMICO] Instrução da vez

1. CHIEF: `update_task_status("WORKING", …)` se for executar.
2. SPECIALIST: `search_ast_symbol` → diff mínimo.
3. CHECKER: `run_verification` → exige `ok:true` / PASS.
4. Sucesso → `READY_FOR_REVIEW`. Falha com o mesmo hash → o MCP já marcou `BLOCKED`.

## [PROIBIÇÃO]

- Proibido ler diretórios/ficheiros em massa sem `search_ast_symbol`.
- Proibido declarar sucesso sem PASS de `run_verification`.
- Proibido "desbloquear" o breaker com `git checkout .`/`git restore .`/`git stash`
  para o verify voltar a correr: isso apaga trabalho alheio (lição 34).

## [CIRCUITO]

Ao receber `BLOCKED` / `circuit_breaker: true` / "LOOP DETECTADO":

1. Interromper no mesmo instante (zero builds, zero patches).
2. Emitir **exatamente 3 linhas** com o diagnóstico do bloqueio.
3. Aguardar humano — o reset do diff é decisão dele.
