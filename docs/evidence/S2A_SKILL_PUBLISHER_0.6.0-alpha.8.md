# Evidence ? S2a assinatura de publicador + allowlist

**Data:** 2026-10-27  
**Estado:** implementado e provado (`s2_tests` + workspace verde).

## O que mudou

- `skill.install` aceita `publisher` (hex da pubkey Ed25519) e `sig` (base64)
  sobre a mensagem can?nica **`name|focus|version|hash`**.
- `verify_publisher` corre **antes de qualquer escrita**:
  - sem `publisher`/`sig` ? `provenance="local"` (install local, sem autor);
  - assinatura inv?lida ? **recusa**;
  - `NOSD_SKILL_PUBLISHERS` n?o vazia e publisher fora ? **recusa**;
  - v?lida e autorizada ? `provenance="signed"` (30 pontos no score).
- O manifesto (`skill/<n>/meta`) passa a registar a proveni?ncia calculada.

## Provas

| Teste | Resultado |
|---|---|
| `s2_tests::signed_manifest_is_accepted_and_tamper_rejected` | assinatura v?lida ? `signed`; manifesto adulterado (vers?o diferente) ? `Err`; sem sig ? `local` |
| `cargo test --workspace` / `clippy -D warnings` | verdes |

## Honestidade / residual (S2b)

- **`caps` declaradas** continuam vazias: o manifesto tem o campo, mas o host
  n?o valida nem restringe por declara??o (o sandbox j? ? zero-caps).
- **Sem hist?rico local** (sucesso/falha) no score ? sempre 0.
- **Sem vers?o monot?nica**: instalar uma vers?o antiga continua permitido; o
  `skill.explain` escolhe a maior.
- **Push n?o assina**: `provenance="peer"` (18) fica abaixo de `signed` (30),
  mas um peer autorizado ainda pode empurrar m?dulo sem autor nominal.
- **Allowlist por vari?vel de ambiente** (root-controlled), sem CRDT/HITL de
  revoga??o.
