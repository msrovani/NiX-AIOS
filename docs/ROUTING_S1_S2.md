# Routing S1/S2 — desenho (V10)

**Pergunta:** quem responde a este pedido — o **0.8B em CPU (S1)** ou o **4B em
GPU (S2/PRO)**? A resposta não pode ser "o maior que existir": tem de ser
coerente com o que a máquina **pode** naquele instante.

## 1. Separação de responsabilidades

| Camada | Pergunta | Fonte |
|---|---|---|
| `score_intent` / `route_intent` | *que tipo de pedido é* (greet, hw, rust, complexo) | heurística pura (µs) |
| `nos-core::routing::plan_backend` | *quem pode responder agora* | estado observado |
| `nos-hermes` | executa a decisão (S1 direto / fila S2) | runtime |
| `route.explain` | mostra lane **e** plano + razão | observabilidade |

O classificador não sabe se o 4B está vivo; o router não sabe se a pergunta é
de código. Juntá-los num só sítio foi o erro da versão anterior (a lane S2
enfileirava mesmo sem S2, e o chat ficava à espera de um worker em CPU).

## 2. Entradas (todas observadas, sem I/O novo)

| Input | Como |
|---|---|
| `lane` | `route_intent(prompt)` |
| `prompt_chars` | `chars().count()` |
| `interactive` | `chat` (sync) vs `chat.submit`/job |
| `s2_available` | `hotswap_s2.is_some()` (attach validado no boot) |
| `s2_pending` | `S2Queue::pending()` |
| `power_save` | `NOSD_PRESENCE` ∈ {sleep, drowsy} |
| `s1_available` | `engine_name() != "cpu-echo"` |

**Porque não sondar VRAM por pedido:** `nvidia-smi` custa ~100 ms e o chat não
pode pagar isso. A admissão de VRAM é feita **uma vez**, no arranque do
`nosd-engine-s2` (o `llama-server` falha se não couber). O router confia nisso:
se o serviço está anexado, há VRAM; se não está, `s2_available=false`.

## 3. Tabela de decisão (ordem = prioridade)

| # | Condição | Backend | Razão | Downgrade |
|---|---|---|---|---|
| 1 | lane S1 | S1 (ou CpuEcho) | `lane_s1` | não |
| 2 | lane S2, `!s2_available` | S1 | `s2_absent` | **sim** |
| 3 | `power_save` | S1 | `power_save` | **sim** |
| 4 | `prompt_chars > 24 000` | S1 | `prompt_too_long` | **sim** |
| 5 | interativo e `s2_pending > 0` | S1 | `s2_busy_interactive` | **sim** |
| 6 | resto | S2 | `lane_s2` | não |

Racional:
- **2** — fail-closed: nunca fingir S2. `CpuEcho` só se nem S1 existe.
- **3** — `Sleep/Drowsy` = libertar VRAM (I3); o S2 sai do caminho.
- **4** — o S2 tem 4k de contexto; acima de ~6k tokens o resultado degrada mais
  do que o 4B ganha em qualidade. (Corte de prompt é trabalho futuro.)
- **5** — interatividade > qualidade: um chat que espera 10–30 s pela fila do 4B
  parece "morto". Jobs (`interactive=false`) **podem** esperar.
- **6** — o caso normal: pergunta profunda com GPU livre vai ao 4B.

## 4. O que isto garante (e o que não)

Garante: nenhum pedido é enviado a um backend inexistente; nenhum chat fica
preso na fila do 4B; presença em sleep não mantém VRAM ocupada; a decisão é
reproduzível e testada (10 testes de tabela em `nos-core::routing`).

Não garante: latência (o 4B em 3050 faz ~10–20 tok/s); qualidade (4B ainda
alucina — a persona manda dizer "não sei"); admissão por VRAM **fininha** (uma
segunda GPU/processo pode comer a VRAM depois do arranque); offload para a
malha (M5, residual).

## 5. Observabilidade

```
nosctl route "explique o scheduler do kernel"
  lane=s2 reason=... plan.backend=S2 plan.reason=lane_s2 downgraded=false
nosctl health | grep -E 'engine|engine_s2'
```

Se `plan.reason=s2_absent` num portátil com 3050: o serviço `nosd-engine-s2`
não subiu (driver/VRAM) — o journal dirá porquê. Se `s2_busy_interactive`:
fila ocupada; mandar um job é o caminho.

### No ecrã (cinematic ORB)

`TELEMETRIA` mostra sempre três linhas: **S2 / PRO** (`engine_s2` ou `ausente`),
**GPU** (`usado/total MiB` do `nvidia-smi`, ou o erro) e **S2 PORTA**
(`127.0.0.1:8082` a responder/sem resposta). O drawer `DIAG` tem as seis linhas
completas (SMI, NGL/CUDA, GPU, S2, porta, SOUL). Uma foto do cockpit resolve o
diagnóstico sem shell.

### Raciocínio desligado na origem

O `llama-server` arranca com `--reasoning off` (spawn do S1 e unit do S2): o
Qwen3.x responde directo em vez de gastar `n_predict` em `<think>`. Isto é a
correcção de raiz; `strip_think` em `nos-core::text` fica como rede de
segurança para servidores antigos. `NOSD_LLAMA_REASONING=on|auto` re-liga.

## 6. Próximos passos (não agora)

1. **Admissão VRAM com cache** (TTL 30 s) para desligar S2 se outro processo
   ocupar a GPU (`nvidia-smi --query-gpu=memory.free` no `observe`).
2. **Escalonamento S1→S2**: se o S1 devolver recusa/vazio em pergunta
   complexa, re-tentar no S2 (uma vez, com log).
3. **Custo/latência previstos**: usar `reported_rate` para decidir por
   orçamento (tokens estimados × tok/s medido) e não só por lane.
4. **Offload de frota**: S2 remoto via `job.submit` quando a GPU local está
   ocupada; hoje é residual honesto.
