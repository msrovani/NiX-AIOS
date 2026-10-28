# Evidence - S2c downgrade via HITL

**Data:** 2026-10-27
**Estado:** implementado e provado (`ipc_auth`, 17 testes).

## Regra

- Instalar versao **inferior** a maxima do mesmo `focus` continua a ser
  recusado por omissao (S2b).
- A excepcao exige um **grant HITL server-authored** com kind
  `skill.downgrade` (`ui.open --grant-kind skill.downgrade` + `ui.action`), o
  mesmo mecanismo do S-04 - **nao** um booleano do cliente.
- CLI: `nosctl skill-install ... --confirm-id <id>`.

## Prova

| Teste | Resultado |
|---|---|
| `skill_downgrade_requires_hitl_grant` | instala 2.0.0; 1.0.0 sem grant -> recusa com "downgrade"; com grant `skill.downgrade` -> aceita |
| restantes `ipc_auth` | 16 continuam verdes |

## Honestidade

- O grant e **single-use** (consumido no install); um downgrade repetido exige
  novo grant.
- Nao fica registado **quem** autorizou (o grant nao e auditado com identidade no
  log) - S2d.
- Sem e2e em VM (o caminho de grant ja tem e2e em S-04/Voice; este usa a mesma
  primitiva).
