# HITL lab (dev/test)

Em **dev/test**, a cerimónia HITL pode ser menos rígida — **opt-in**, nunca
default na ISO.

## Activar

```bash
export NOSD_HITL_LAB=1
# ou
export NOSD_ENV=dev   # também: test | lab
```

## O que relaxa

| Controlo | Prod (sem env) | Lab |
|----------|----------------|-----|
| `confirm_id` server-authored | obrigatório | auto-ok |
| CapGate bit `HITL` | exigido | mascarado (`without_hitl`) |
| `hitl_confirmed` do cliente | **ignorado** | **ignorado** |
| NIX_WRITE / ring / FS / NET | intactos | intactos |

## Honesty

Boot log: `HITL LAB SOFT … NÃO usar em ISO/prod`.

Testes de contrato IPC (`ipc_auth`) **não** definem o env → continuam a
provar o caminho fail-closed.
