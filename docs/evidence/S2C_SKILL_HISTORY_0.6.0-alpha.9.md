# Evidence - S2c historico no score (fatia 1)

**Data:** 2026-10-27
**Estado:** implementado e provado (`ipc_auth` 16 PASS).

## O que mudou

- `skill.run` regista o resultado em `skill/<nome>/stats` = `[ok, fail]` (CRDT).
- `skill.explain` le esses contadores e soma ao score:
  `history = 20*ok/(ok+fail)` com **>= 5 execucoes**; com menos, `8` (neutro,
  nao premeia nem pune).
- O `why` passa a mostrar `history` e `runs: [ok, fail]` - a decisao do Hermes
  fica reproduzivel.

## Prova

| Teste | Resultado |
|---|---|
| `skill_history_feeds_the_score` | `good` corre 5x (ok) e `bomb` 5x (fuel); empatam em proveniencia/caps/versao/tamanho e o **historico desempata**: `good@1.0.0`, `history=20`, `runs=[5,0]` |
| restantes `ipc_auth` | 15 continuam verdes |

## Honestidade / residual (S2c restante)

- **Historico e local** (por no), nao e reputacao global - de proposito.
- **Sem push assinado**: um peer continua a entrar como `provenance=peer` sem
  autor nominal.
- **Sem revogacao** de publisher/skill por CRDT/HITL.
- **Sem downgrade via HITL** (continua recusado sem excepcao).
- O historico **nao decai** (uma skill que falhou ontem pesa igual hoje).
