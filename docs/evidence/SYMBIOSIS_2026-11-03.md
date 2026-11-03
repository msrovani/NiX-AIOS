# Simbiose Nix ↔ AIOS — fábrica real, domínio Nix e HUD consciente (2026-11-03)

Versão de pacote: **0.7.0-alpha.5** (in-tree; sem bump).
Âmbito: os três pontos pedidos — (a) fábricas de app que **funcionam** em Rust e
Python, (b) córtex/hermes/JARBAS a **dominar o Nix** desta máquina, (c) GUI a
expandir essa consciência.

## 1. Fábrica de apps reais (`nos-factory::scaffold`)

Antes: `factory.plan` devolvia 3 templates WASM com JSON fixo (placeholders
declarados) e `SkillOpt` escrevia um draft vazio substituído pelo módulo
identidade. Não era uma fábrica de apps: era um catálogo de stubs honestos.

Agora (`crates/nos-factory/src/scaffold.rs`):

| Peça | Contrato |
|---|---|
| `plan(intent, lang)` | escolhe **Rust** (default) ou **Python** pelo intent; gera `main.rs`/`Cargo.toml` (ou `app.py`) + `flake.nix` + `README.md`; `content_id` = BLAKE3 de (path+conteúdo ordenados) |
| `write(plan, root)` | escreve em `<root>/<name>`; nome `[a-z0-9-_]`, recusa `..`/absolutos e **recusa alvo symlink** |
| `build_and_run(plan, dir)` | Rust: `cargo run` se existir, senão `rustc -O main.rs -o app`; Python: `python3 app.py`; timeout 120 s (30 s python), morte do processo no prazo |
| `RunOutcome.status` | `ok` \| `build_failed` \| `run_failed` \| `engine_absent` \| `timeout` — **nunca** sucesso sem stdout real |

RPC:

- `factory.scaffold` (caps `NONE`) — plano com ficheiros inline, **não escreve**.
  É o que o kiosk pode chamar.
- `factory.build` (caps `AGENT_SPAWN|FS`) — escreve no `data-dir/factory/apps`,
  constrói, corre e grava OPAR; `ok=false` quando o estado não é `ok`.
  Fora da allowlist do kiosk (o browser não compila nem escreve).

CLI: `nosctl factory-scaffold <intent> [--lang]`, `nosctl factory-build <intent> [--lang]`.

O `flake.nix` gerado constrói a app no Nix (Rust: `stdenv` + `rustc`; Python:
`writeShellApplication` com `python3`) — a app nasce Nix-native, não só "um
ficheiro que corre".

## 2. Domínio Nix (`nos-core::nix_domain`)

Duas metades, uma fonte:

1. **Factos vivos** (`probe()`): `/etc/NIXOS` (versão), `/run/current-system`
   (closure), `/etc/nixos/{flake,configuration}.nix`, contagem de
   `system-NN-link` em `/nix/var/nix/profiles`, `NOSD_NIX_RING`,
   `/run/systemd/system`. Ausente = `null` (honesty ADR-0113), nunca default
   inventado.
2. **Lições caras** (`LESSONS`, 20 entradas): destilado operacional do
   `AGENTS.md` — árvore git do flake, `path`/`StartLimit` no unit, `char-nvidia`
   no `DeviceAllow`, CRLF/BOM no `.nix`, `${VAR}` dentro de `''…''`,
   `mkForce` vs `mkOverride`, `cargoBuildFlags`, `WatchdogSec`, ring desarmado,
   QEMU ≠ metal. Cada lição tem `triggers` (não substring solta) e `lookup()`
   é determinístico (score, depois id).

Consumidores:

- **Prompt do córtex/hermes**: `cognitive_bridge::build_chat_prompt` acrescenta
  `## Nix (live)` (factos + até 3 lições, tecto 900 chars) **antes** de
  `## User`, e só quando a pergunta toca no Nix — perguntas sem relação não
  gastam contexto de um 0.8B.
- **Roteamento**: `nos-cortex::route` ganhou `nix_keywords` — consulta fica em
  S1 (`Active`), mudança/erro (`nix build`, `switch`, `apply`, falha) sobe a
  S2 (`GeneratorPro`). Antes, `nix build` com erro caía em `default_s1`.
- **RPC**: `nix.status` e `nix.ask` (caps `NONE`), expostos ao kiosk.
- **CLI**: `nosctl nix-status`, `nosctl nix-ask "<pergunta>"`.

## 3. HUD (ORB web) — consciência de simbiose

`static/orb_lab.html` ganhou o painel **02c SIMBIOSE**:

- `NIX` (versão/`HOST`), `GERAÇÃO`, `RING` (ARMED/DISARMED), `LIÇÕES`,
  `JANELAS` (janelas + grants pendentes), alimentado por `orb.symbiosis`
  (caps `NONE`, leitura pura).
- Campo de intent + botões `RUST` / `PY` → `factory.scaffold` → mostra nome,
  `content_id`, toolchain disponível/ausente e ficheiros. O rodapé do painel
  diz explicitamente que nada é escrito sem decisão humana.

## Verificação

- `cargo test --workspace` — ver `Tested:` no commit.
- `bash scripts/smoke-symbiosis.sh` contra daemon local (TCP + token):
  `nix-status`, `nix-ask` (lição `flake-git-tree`, e `match: 0` para pergunta
  sem relação), `symbiosis`, `factory-scaffold` rust/python,
  `factory-build` rust/python com `status: ok`.
- Contratos: `ipc_registry` (caps `NONE` de `nix.*`/`factory.scaffold`),
  `kiosk_methods_are_scope_limited` (novos métodos `NONE`; `factory.build`
  proibido no kiosk).

## Not-tested

- Metal/QEMU: o painel não foi visto num cage/Chromium real (só bancada web).
- `facter`/`nixos-version` externos: a sonda lê ficheiros, não executa `nix`.
- `factory.build` com toolchain ausente: coberto no código (`engine_absent`) e
  nos unit tests, mas este host tem `cargo`/`python3` — o ramo "sem toolchain"
  não foi exercitado num ambiente real só com o daemon.
- Build Nix das apps geradas (`nix build` na `flake.nix` do projecto gerado):
  o ficheiro é gerado e testado por conteúdo, não construído no CI.
