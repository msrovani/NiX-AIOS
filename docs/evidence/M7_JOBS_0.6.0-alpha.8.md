# Evidence — M7 jobs remotos (frames assinados sobre TCP)

**Data:** 2026-10-27  
**Estado:** implementado e provado em VM; entra na release de frota (alpha.9) —
não tagged.  
**Check:** `checks.x86_64-linux.mesh-2vm-jobs`

## O que foi construído

| Peça | Função |
|---|---|
| `nos-mesh::job` | envelope `JobEnvelope`, `JobResult`, frame `NJOB|ver|len|body|sig` |
| `Pinned::check_key` | confiança partilhada mesh↔jobs (chave fixada/TOFU) |
| `nosd` `NOSD_JOB_BIND` | listener TCP opt-in |
| RPC `job.submit` | encaminha para `addr`, verifica assinatura do worker |
| `nosctl job-submit <addr> <prompt>` | cliente |

### Propriedades

- **O worker tem de ser peer conhecido**: o job só é aceite se a chave do
  cliente estiver fixada no mesh (ou TOFU) e o id derivar dessa chave.
- **Resultado assinado**: o cliente verifica a assinatura do worker e recusa
  chave diferente da fixada (`worker nao autorizado`).
- **Limites**: prompt ≤ 8 KiB, corpo ≤ 1 MiB, deadline 0.1–120 s (clamp),
  timeout de ligação/leitura = deadline + 2 s.
- **Sem root/efeitos**: o executor só chama `chat_submit` com
  `INFER|MEMORY` (gate local do worker), nunca o ring, nunca `/etc`.

## Teste (`mesh-2vm-jobs`)

2 VMs com mesh assinado (`NOSD_MESH_REQUIRE_SIG=1`) e B com
`NOSD_JOB_BIND=0.0.0.0:19001`:

| Asserção | Resultado |
|---|---|
| chaves fixadas (`mesh_pinned: 1`) antes do job | PASS |
| `job listener bind=` no journal de B | PASS |
| A (sem modelo) → `job-submit B 'oi do no A'` | `verified: true`, `ok: true` |
| prompt de 9 KiB | recusado (`prompt acima do limite`) |
| porta fechada (`:19999`) | falha de ligação |

## Honestidade / residual

- **Sem confidencialidade**: frames assinados mas em claro (Noise = M6.1).
  Quem estiver na mesma LAN vê prompts.
- **Sem scheduler distribuído**: o cliente escolhe `addr`; não há fila, retry,
  nem balanceamento. `MESH_OFFLOAD_HINT` continua por consumir.
- **Quota parcial**: limites de tamanho/tempo; falta quota por peer
  (tokens/s, jobs concorrentes) e idempotência por `job_id` no servidor.
- **Resultado não é verificável** além da assinatura (o worker pode mentir) —
  TEE/attestação continua `AWAITING_HW`.
- O executor é o `chat` local (cpu-echo/llama-server); não há ainda
  `infer.remote` como método separado nem streaming de tokens.
