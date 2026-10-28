# Evidence — M9 skills wasmi (sandbox frugal)

**Data:** 2026-10-27  
**Estado:** implementado e provado (testes de integração com daemon real).  
**Fase:** M9 (fatia 1 — registry + execução local; distribuição fica para M9.1).

## O que foi construído

| Peça | Função |
|---|---|
| RPC `skill.install` | aceita `wat` (compilado no servidor) ou `wasm_b64`; valida antes de gravar |
| storage | `<data-dir>/skills/<fnv1a64>.wasm` (content-addressed) |
| registo | `skill/<nome>/active = hash` no CRDT (troca = op; rollback = hash anterior) |
| RPC `skill.list` / `skill.run` | lista e executa o export `run: i32 -> i32` |
| `nosctl skill-install/skill-list/skill-run` | CLI |

### Sandbox

Herda o runtime de `nos-wasm`: **fuel 100k**, heap **256 KiB** (4 páginas),
1 memória/tabela/instância, e só o import `aios_ai::infer` — que devolve `-1`
porque a skill corre com `CapBits::NONE`. Sem fs, sem rede, sem `exec`.

## Testes (14 PASS em `ipc_auth`)

| Teste | Prova |
|---|---|
| `skill_install_run_and_list` | instala WAT, `run(7) == 7`, aparece no `list` |
| `skill_fuel_exhaustion_is_rejected` | `loop br 0` → `ok:false`, erro `fuel` |
| `skill_infer_is_denied_without_cap` | import `infer` sem cap → `-1` |

`cargo clippy -D warnings` e `fmt --check` verdes; `cargo test --workspace` verde.

## Honestidade / residual

- **Fnv-1a64 como id de conteúdo**: serve para dedup, não é commitment
  resistente a colisão. BLAKE3 entra no M8.1.
- **Sem distribuição**: a skill instala-se e corre **no nó local**; não há
  `SKILL_PUSH` para peers nem execução remota. O CRDT já propaga a chave
  `skill/<n>/active` (via `crdt.sync`), portanto a *activação* já é
  replicável; o **binário** ainda não.
- **Validação = executar uma vez** com arg 0: um módulo com efeitos colaterais
  (não há I/O disponível) ou muito lento gasta fuel na instalação.
- **Sem quota por skill** (tempo/fuel por chamada é fixo) nem assinatura de
  módulos; a confiança é "quem tem `FS` pode instalar" (LocalAdmin).
- Continua **sem Noise** (M6.1): o estado `skill/<n>/active` viaja legível.
