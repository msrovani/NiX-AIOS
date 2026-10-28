# Evidence — M8 estado replicado (CRDT sobre link autenticado)

**Data:** 2026-10-27  
**Estado:** implementado e provado; entra na release de frota (alpha.9).  
**Check:** `checks.x86_64-linux.mesh-2vm-crdt`

## O que foi construído

| Peça | Função |
|---|---|
| `JobEnvelope.kind` | 1 = chat, 2 = crdt |
| `nos_mesh::job::CrdtHook` | o servidor mergeia o payload e devolve o snapshot |
| `submit_crdt` | cliente de anti-entropy no mesmo link assinado do M7 |
| `MeshEngine::crdt_put/snapshot/merge/len` | API de estado no engine |
| RPC `crdt.put` / `crdt.get` / `crdt.sync` | `nosctl crdt-put/crdt-get/crdt-sync` |

Só peers com **chave fixada** entram (herança do M6/M7). O merge é o existente:
LWW por version vector, desempate `(tick, writer)`.

## Teste (`mesh-2vm-crdt`)

2 VMs, mesh assinado, ambos com `NOSD_JOB_BIND=0.0.0.0:19001`:

| Passo | Resultado |
|---|---|
| A: `crdt-put k1 v1` + `crdt-sync → B` | `verified: true` |
| B: `crdt-get k1` | `value: v1` |
| B: `crdt-put k2 v2` + `crdt-sync → A` | A vê `k2=v2` |
| `entries` nos dois | **2** (convergência) |

## Honestidade / residual

- **Anti-entropy de estado completo** (v1): envia o store todo; falta delta por
  version vector, Merkle-DAG e bloom filter (o plano prevê; não está feito).
- **Transporte**: frames assinados mas **em claro** (Noise = M6.1). Estado de
  frota (nomes, skills, papéis) viaja legível na LAN.
- **Sem persistência**: o CRDT vive em memória; `nsgdb` ainda não guarda o log
  de operações (isso é o que dá durabilidade/replay).
- `crdt.put` exige `MEMORY` (LocalAdmin no CLI); não há quota por peer.
