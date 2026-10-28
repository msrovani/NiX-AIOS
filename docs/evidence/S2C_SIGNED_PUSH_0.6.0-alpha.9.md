# Evidence - S2c push assinado

**Data:** 2026-10-27
**Estado:** implementado; **payload testado**, ligacao no destino NAO tem teste e2e.

## Fluxo

```
A: skill.install (publisher+sig validos) -> meta guarda publisher/sig
A: skill.push -> payload inclui focus/version/publisher/sig   (push_payload)
B: verify_publisher(name,focus,version,hash,pub,sig)
     ok    -> provenance = signed
     erro  -> push RECUSADO (nao cai para "peer")
     ausente -> provenance = peer
B: install_bytes(hash) -> put_meta(provenance, focus, version)
```

## Provas

| Teste | Cobertura |
|---|---|
| `s2c_push_tests::push_payload_carries_authorship` | o payload leva publisher/sig/focus/version; sem manifesto nao inventa autoria |
| `s2_tests::signed_manifest_is_accepted_and_tamper_rejected` | verificador (valida/adulterado/sem sig) |
| `clippy -D warnings`, restantes testes | verdes |

## Honestidade

- **A ligacao no destino nao tem teste e2e**: nao ha chave/assinatura no check
  `mesh-2vm-skills`, portanto o caminho `provenance=signed` via push esta
  implementado mas **nao provado em VM**. O verificador em si e testado.
- O push **sem** assinatura continua a entrar como `peer` (autorizado pelo
  pinning do canal, sem autor nominal).
- Falta: revogacao (CRDT/HITL) e downgrade via HITL.
