# Evidence ? S2b caps declaradas + vers?o monot?nica

**Data:** 2026-10-27  
**Estado:** implementado e provado (workspace + clippy verdes).

## O que mudou

| Antes (S2a) | Agora (S2b) |
|---|---|
| `caps` do manifesto sempre `[]` | `caps` declaradas e **validadas** contra allowlist fechada |
| ? | `aios_ai.infer`, `log` s?o as ?nicas permitidas; qualquer outra = **recusa** |
| instalar vers?o antiga permitido | **downgrade recusado** por `focus` (fail-closed) |
| score lia `caps` (sempre 0) | score continua `20 ? 4?n` (agora com valor real) |

`verify_caps` e a compara??o semver s?o puros e testados ? parte.

## Provas

| Teste | Resultado |
|---|---|
| `s2b_tests::declared_caps_are_restricted_to_allowlist` | `[]` e `[aios_ai.infer, log]` ok; `fs.read`/`net.connect` ? `Err` |
| `s2b_tests::semver_key_orders_versions` | `2.0.0 > 1.9.9`, `1.10.0 > 1.9.0`, `1.2 == 1.2.0` |
| `cargo test --workspace` / `clippy -D warnings` | verdes |

## Honestidade / residual (S2c)

- **`caps` s?o declarativas**: o host continua a **n?o expor** nada al?m de
  `aios_ai.infer` (que devolve -1 sem `INFER`). A valida??o impede manifesto
  mentiroso, n?o implementa capacidades novas.
- **Sem hist?rico** (sucesso/falha) no score ? continua 0.
- **Push n?o assina** (`provenance=peer`): um peer autorizado n?o precisa de
  publisher nominal.
- **Sem revoga??o** de publisher/skill por CRDT/HITL.
- **Downgrade recusado** sem excep??o (n?o h? caminho HITL para o permitir).
