# Evidence - S2c revogacao de publisher

**Data:** 2026-10-27
**Estado:** implementado e provado (unit).

## Mecanismo

- Estado: chave CRDT `skill/revoked/<publisher_hex>` (valor livre, presenca = revogado).
- `verify_publisher` passou a receber o conjunto de revogados e **recusa mesmo com
  assinatura valida** (fail-closed, depois da allowlist).
- Aplicado nos dois caminhos: `skill.install` e o hook do `skill.push`.
- Como revogar hoje: `nosctl crdt-put skill/revoked/<hex> 1` (replica pela frota
  via `crdt.sync`). Um subcomando dedicado fica para S2d.

## Prova

| Teste | Resultado |
|---|---|
| `s2c_revoke_tests::revoked_publisher_is_refused_even_with_valid_signature` | sem revogacao -> `signed`; com o publisher revogado -> `Err` contendo "revogado" |
| `s2_tests` / `s2c_push_tests` | verdes (assinatura continua a validar) |

## Honestidade

- **Sem CLI dedicado** (`skill-revoke`) nem HITL: escrever a chave exige `crdt-put`
  (LocalAdmin). Revogar em frota replica, mas nao ha auditoria do "quem revogou".
- **Sem e2e em VM** (nao ha assinatura no check) - o mecanismo e testado ao nivel
  da funcao que decide.
- **Sem expiracao** de revogacao nem rede de seguranca para reverter um erro
  (remover a chave e um `crdt.put`).
- Historico continua local e sem decaimento; downgrade via HITL continua em falta.
