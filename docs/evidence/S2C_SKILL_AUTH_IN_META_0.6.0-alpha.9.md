# Evidence - S2c autoria persistida no manifesto (fatia 2)

**Data:** 2026-10-27
**Estado:** implementado e provado (`s2c_tests`).

## O que mudou

Quando `skill.install` traz `publisher`+`sig` validos, a assinatura deixa de
ser consumida e descartada: fica **guardada no manifesto**
(`skill/<nome>/meta` -> `publisher`, `sig`), via `meta_with_auth`.

Isto e o pre-requisito do **push assinado**: sem a assinatura no manifesto, o
no que reencaminha a skill nao tem como provar autoria ao destino (hoje o
destino so consegue marcar `provenance=peer`).

## Prova

| Teste | Resultado |
|---|---|
| `s2c_tests::meta_records_publisher_and_signature_when_signed` | com publisher+sig o manifesto fica com ambos; sem eles fica intacto |
| `s2_tests` / `ipc_auth` | continuam verdes |

## Residual (S2c restante)

- **Push ainda nao reencaminha** publisher/sig (o manifesto ja os tem; falta
  incluir no payload e verificar no destino -> `provenance=signed`).
- **Sem revogacao** por CRDT/HITL.
- **Sem downgrade via HITL**.
- Historico local e sem decaimento.
