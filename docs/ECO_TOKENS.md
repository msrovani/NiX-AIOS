# Eco-Tokens (MCP + regras) — instalação local

Instalação do kit [msrovani/Eco-tokens](https://github.com/msrovani/Eco-tokens)
v0.3.0 neste repositório pelo **`install.sh` oficial**, depois endurecido com um
patch local e ligado a OpenCode. Data: 2026-10-29.

## Como foi instalado

```bash
git clone --depth 1 https://github.com/msrovani/Eco-tokens   # kit local
bash install.sh --target /c/DEV/nix-aios                      # instalador OFICIAL
# → alvo=/c/DEV/nix-aios stack=unknown python=python3  (sem Cargo.toml na raiz)
bash install.sh --self-test                                   # SELF_TEST_OK 0.3.0
cd .eco && git -c core.autocrlf=false apply -p2 nixaios.patch  # hardening local
```

O `install.sh` correu sem `--force`, por isso manteve `.eco/state.json` (estado
compartilhado com outra sessão). Depois do install foram repostos dois ficheiros
que o instalador escreve de forma inadequada a este repo:

- `scripts/verify.sh` — o instalador gerou `stack=unknown` (que falha sempre);
  substituído pela versão NixAIOS (workspace `packages/nosd`).
- `.eco/mcp-server.py` — ficheiro do upstream + `.eco/nixaios.patch`
  (reprodutível: aplicar o patch a uma cópia crua dá byte-a-byte o instalado).
  `git apply` com `core.autocrlf=true` escreve CRLF; usar
  `git -c core.autocrlf=false apply -p2`.

## O que existe no repo

| Caminho | Papel | Versionado |
| --- | --- | --- |
| `.eco/mcp-server.py` | servidor MCP stdio, Python stdlib, 4 tools (upstream + patch) | sim |
| `.eco/nixaios.patch` | hardening local, reaplicável após re-correr `install.sh` | sim |
| `.eco/RULES.md` | regras CoD + anti-loop, carregadas como `instructions` | sim |
| `.eco/state.json` | estado da tarefa ativa (atómico, com `.bak`) | não (gitignore) |
| `scripts/verify.sh` | verificação: fmt + clippy + test em `packages/nosd` | sim |
| `opencode.json` | `mcp.servers.eco-tokens` + `instructions: [".eco/RULES.md"]` | sim |
| `.cursorrules`, `.windsurfrules`, `.cursor/mcp.json`, `.tasks.json` | escritos pelo instalador oficial (Cursor/Windsurf + legado) | sim |

Tools expostas como `eco_tokens_*` (Code Mode: `tools.eco_tokens.get_current_task(...)`):
`get_current_task`, `update_task_status`, `run_verification`, `search_ast_symbol`.

## Desvios ao upstream (deliberados)

1. **Sem revert automático.** O breaker do upstream corre `git checkout .` ao
   detetar loop. Aqui **recusa** (`circuit_breaker: true`, `reverted: false`,
   `revert_policy: "refuse-no-revert"`) — AGENTS.md lição 34: o revert apaga
   trabalho de outra sessão por causa de teste flaky.
2. **UTF-8 forçado em stdin/stdout.** Em Windows o `tools/list` morria com
   `UnicodeEncodeError` (cp1252 vs `→` e `…`).
3. **`find_bash` prefere Git Bash.** `bash` no PATH do Windows é o WSL
   (`C:\windows\system32\bash.exe`), que devolve 127 a um caminho Windows; o
   verify morria a 127. `ECO_BASH` continua a mandar (definido em `opencode.json`).
4. **`search_ast_symbol` com fallback stdlib.** Sem `ast-grep`/`rg`/`grep` no
   PATH (host Windows), passou a haver varredura Python (`tool: "python"`) em vez
   de `tool: "none"` — mantém o teto de 25 linhas e salta `.git`/`target`/`.eco`.
5. **`scripts/verify.sh` stack-aware do repo**, não `unknown`: o workspace é
   `packages/nosd` (não há `Cargo.toml` na raiz). Passos = os comandos de
   `AGENTS.md`. Saltos explícitos: `ECO_SKIP_CLIPPY=1`, `ECO_SKIP_TEST=1`
   (marcados no log como `SKIP`, para a verificação não mentir).
6. `.cursorrules` / `.windsurfrules` / `.cursor/mcp.json` ficam como o instalador
   os escreveu (não são usados aqui: as regras vivem em `.eco/RULES.md` e
   `opencode.json`, que é o que o OpenCode carrega).

## Uso

```text
get_current_task()                      # começar por aqui
update_task_status(WORKING, "motivo")   # antes de editar
run_verification(dry_run=true)          # fingerprint SHA256(git diff) sem build
run_verification()                      # fmt + clippy -D warnings + cargo test
```

- Fingerprint igual **depois de uma falha** → `BLOCKED`, sem build. Parar, não
  patchar; o reset do diff é decisão humana.
- `dry_run` nunca trip o breaker (só consulta).
- Logs passam por `smart_trim`: ANSI removido, >20 linhas → 5 head + 15 tail,
  teto ~1000 chars.

## Verificação feita (2026-10-29, host Windows)

- `bash install.sh --self-test` (kit oficial) → `SELF_TEST_OK 0.3.0`.
- Patch reprodutível: `git -c core.autocrlf=false apply -p2` sobre a cópia crua do
  upstream reproduz `.eco/mcp-server.py` byte-a-byte.
- `py_compile` + handshake MCP (`initialize` 0.3.0, `tools/list` = 4 tools) PASS.
- `run_verification(dry_run=true)` PASS; `search_ast_symbol("LoopGuard")` devolve
  15–16 linhas via fallback Python.
- Anti-loop em sandbox (`git init` + `verify.sh` que falha): 1.ª falha `ok:false`;
  2.ª com o mesmo hash `circuit_breaker:true`, `reverted:false`, `BLOCKED`, e o
  ficheiro modificado **sobreviveu** (prova de que não houve `git checkout .`).
  No sandbox, sem gitignore de `.eco/state.json`, o fingerprint churnava a cada
  escrita do estado — daí o estado ficar fora do git aqui.
- `scripts/verify.sh` com `ECO_SKIP_CLIPPY=1 ECO_SKIP_TEST=1` → `VERIFY_OK`.
- Verify **completo** no host (`fmt --all --check` + `clippy --workspace
  --all-targets --all-features -- -D warnings` + `cargo test --workspace` em
  `packages/nosd`) → `VERIFY_OK stack=rust`, rc=0 (2026-10-29).
- Chamada real da tool MCP `eco_tokens_run_verification` nesta sessão (dry_run +
  a versão que corre o mesmo `verify.sh`) — ver `output` do estado `.eco/state.json`.

## Não testado / limites

- Sem `nix build`, sem QEMU, sem metal: só host Windows + Git Bash.
- O breaker continua a ser advisory: um humano (ou outro agente) pode ignorar
  `BLOCKED` e patchar; o MCP não impede edições.
- `.eco/state.json` é local: duas máquinas/sessões não partilham a tarefa ativa.
- `search_ast_symbol` não é semântico (substring), e o fallback Python varre o
  repo — em árvores enormes é mais lento que `rg`.
