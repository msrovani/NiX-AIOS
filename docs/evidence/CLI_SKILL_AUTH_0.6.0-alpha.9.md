# Evidence - CLI de autoria de skills (S2c/S2d)

**Data:** 2026-10-27
**Estado:** implementado (CLI fino sobre primitivas ja testadas).

## Comandos

```bash
# 1) instalar e ver o hash BLAKE3
nosctl skill-install sum /tmp/sum.wat --focus sum --version 1.0.0

# 2) assinar o manifesto com a chave do no (precisa de acesso a
#    /var/lib/nosd/keys/mesh.key -> sudo)
sudo nosctl skill-sign sum /tmp/... --focus sum --version 1.0.0 <hash>
#   -> { "publisher": "<hex>", "sig": "<b64>", "msg": "sum|sum|1.0.0|<hash>" }

# 3) instalar com autoria
nosctl skill-install sum /tmp/sum.wat --focus sum --version 1.0.0   --publisher <hex> --sig <b64>

# 4) revogar um publisher (assinatura deixa de valer)
nosctl skill-revoke <hex>       # escreve skill/revoked/<hex>=1 (MEMORY)
```

## Porque isto desbloqueia o e2e

Antes nao havia forma de produzir uma assinatura no guest, portanto o push
assinado so podia ser testado ao nivel da funcao. Com `skill-sign` o check de VM
passa a conseguir: assinar -> instalar assinado -> empurrar -> verificar
`provenance=signed` -> revogar -> recusar.

## Honestidade

- O CLI e **fino**: assina com `Identity::from_seed` e chama RPCs ja testados.
  Nao ha teste unitario do parser (o e2e em VM e o teste que importa).
- `skill-sign` le a **chave privada do no** (0600, dono `nosd`): exige root e
  assina como "este no". Um modelo de chaves de publicador separado fica para
  depois.
- `skill-revoke` e um `crdt.put` (sem HITL nem auditoria de quem revogou).
- O **e2e no check de VM ainda nao foi escrito** (proximo passo).
