# Evidence - S2c e2e do push assinado (VM)

**Data:** 2026-10-27
**Estado:** **provado em VM** (`mesh-2vm-skills` PASS).

## Cadeia testada (2 VMs, canal Noise)

```
A: skill-install (1.0.0)                 -> hash BLAKE3
A: sudo skill-sign sum <hash> ...        -> publisher + sig (chave do no)
A: skill-install --publisher --sig       -> provenance=signed no manifesto
A: skill-push B                          -> verified=true
B: crdt-get skill/sum/meta               -> provenance == "signed"
B: skill-revoke <publisher>              -> skill/revoked/<hex>=1
A: skill-push B                          -> RECUSADO (a.fail) por revogacao
```

Assercoes no `testScript` (Python, sem grep fragil): `json.loads` do output do
`nosctl`, `assert provenance == "signed"`, e `a.fail(...)` no push apos revogar.

## O que isto fecha

- O push assinado deixou de ser "implementado mas nao provado": a autoria viaja,
  e verificada no destino antes de gravar, e a revogacao corta o caminho.
- Usa o **CLI real** (`skill-sign`/`skill-revoke`), nao assinaturas injectadas no
  teste.

## Residual

- `skill-sign` assina com a **chave do no** (exige root); nao ha chaves de
  publicador separadas nem delegacao.
- Revogacao sem HITL/auditoria (e um `crdt.put`).
- Sem expiracao de revogacao; sem downgrade via HITL; historico sem decaimento.
- HB de descoberta continua em claro; M8.1d (nsgdb) e metal em aberto.
