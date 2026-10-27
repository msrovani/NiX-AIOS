# SKYNET — Frota cognitiva NixAIOS (plano de processamento distribuído)

**Estado:** plano (não implementado). Base: 0.6.0-alpha.8.  
**Âmbito:** M6–M10 — transporte seguro, trabalho remoto, estado replicado,
skills wasm, frota heterogénea.  
**Não-âmbito:** exterminar o futuro. O nome é interno; o produto é uma frota
de máquinas sob controlo humano, com kill switch e HITL em ações destrutivas.

---

## 0. Sumário executivo (decisões que o plano fixa)

1. **Não se faz tensor-parallel (TP) pela rede.** TP vive dentro de um host
   (NVLink/PCIe). Entre hosts usa-se **pipeline (layer split)** em LAN rápida,
   ou **pedido remoto inteiro** em WAN. Isto não é preferência: é física
   (ver §2.1 e §2.2).
2. **Identidade primeiro.** Sem chave de nó assinada, nada de trabalho remoto.
   `Ed25519` + `NoProto` assinado + contador anti-replay; TOFU com pareamento
   fora-de-banda e allowlist explícita. Sem auto-join.
3. **Transporte: Noise IK/XX sobre TCP** (e QUIC opcional para mobilidade).
   Noise porque é P2P sem CA, 1 RTT, auditável; TLS/QUIC fica como alternativa
   quando houver PKI. UDP serve só para descoberta.
4. **O canal entre nós não é chat.** É `NoProto` + CRDT delta + referências de
   KV/prefixo + intenção latente. Prompt é o último recurso (§5 "telepatia").
5. **WASM (wasmi) é a unidade de extensão e de "app swap"**: módulos
   content-addressed, capability-gated, com fuel e quota. Não é o motor de
   inferência.
6. **Segurança é a camada, não uma feature.** Cada fase só fecha com teste
   negativo (MITM, replay, sybil, resultado adulterado, fuel exhaustion).
7. **Honestidade:** 2 VMs numa VLAN não são frota; QEMU não é metal; o plano
   só declara `VERIFIED` com repetição e artefacto.

---

## 1. Ponto de partida real (o que o repo tem hoje)

| Peça | Estado | Ficheiro |
|---|---|---|
| HB UDP (`HB\0` + id + `PK\0` + `CAP\0`) | funciona, **não assinado** | `nos-mesh/src/lib.rs:331` |
| MCH (saúde de máquina) | enviado com o HB após `health` | `encode_tick_extras` |
| `ROLE` (Master/Worker) | eleição stub por capacidade | `run_election_stub` |
| TOFU pk-bound + settle 7.2 s | feito + unit test | `tofu_settles_after_window` |
| `LayaIntent` (NoProto `TaskType=8`) | propaga, mesmo `fnv` nos 2 nós | `laya_intent.rs` |
| CRDT thin (LWW + version vector) | **local**; sem wire/merge | `crdt.rs` |
| `FRAG`/`FRACK` | implementado, **sem chamador** | `udp.rs:63` |
| Offload | **hint local** (`MESH_OFFLOAD_HINT`), sem consumidor | `nos-hermes/src/lib.rs:210` |
| S2 (inferência) | 1 thread local, CAP=8 | `nos-cortex/src/s2_queue.rs` |
| `mesh-2vm` | peers=1, PK, eleição | `checks.x86_64-linux.mesh-2vm` |

Conclusão: existe **descoberta e eleição**, não existe **trabalho**.

---

## 2. O que a investigação diz (e o que obriga a mudar)

### 2.1 Paralelismo: TP vs PP vs offload

- **TP** faz ~2 AllReduce por camada. A 10 KiB por AllReduce e 64 camadas, são
  ~128 sincronizações por token. Com RTT LAN de 0.05–0.1 ms isso já são
  10–25 ms/token — come o orçamento inteiro. Em NVLink são 20–40 µs.
  → **TP só intra-host.**
- **PP (layer split)** troca o hidden state (alguns KB) por fronteira de stage,
  ~1 handoff por token. Tolera RTT de LAN e até WAN com penalidade.
  → **PP é o único modo cross-host são.**
- Hierarquia correta: **TP dentro do worker, PP entre workers**
  (pedido aberto no llama.cpp para o modo híbrido; hoje não existe).

Fonte: `llama.cpp` tools/rpc README, issues #23568/#22850, análise de
`llamacpp-v100/docs/distributed-inference-plan.md`, discussão #15796.

### 2.2 llama.cpp RPC: números e limites

- Só `-sm layer` é prático por rede; `tensor` falha com devices RPC.
- Durante geração só passam ativações (poucos KB) → a rede fica a **5–50 Mbit**
  e **não é a largura de banda o gargalo**: é latência e serialização.
- Medições: −28% a −55% de tokens/s vs local; `prompt processing` −33%.
- **Sem auth/TLS** ("never run on an open network"), **síncrono** (1 comando em
  voo), **sem tolerância a falhas**, sem negociação de capacidades.
- `--rpc` carrega GBs no arranque → precisa de cache local por hash de tensor.

Implicação NixAIOS: não vamos reinventar o transporte do llama.cpp em cima de
UDP broadcast; ou usamos `ggml-rpc` dentro de um túnel Noise, ou fazemos
**pedido remoto** (o worker corre o modelo inteiro) — que é o modo por omissão.

### 2.3 WAN e treino de baixa comunicação

- PETALS faz pipeline sobre a Internet com tolerância a falhas; latência alta,
  mas viável para modelos que não cabem numa máquina.
- **DiLoCo/OpenDiLoCo**: dois otimizadores (inner local, outer periódico) →
  ~500× menos comunicação, robusto a nós lentos. Relevante para "a frota
  aprende", não para servir.
Implicação: separar **servir** (latência) de **aprender** (comunicação esparsa).

### 2.4 KV cache e prefix sharing

- LMCache/Mooncake mostram que reutilizar KV entre nós (prefix caching,
  prefill/decode disaggregation) dá ganhos grandes em cargas agênticas.
- Custo real: 1 token de KV em Llama-3-8B ≈ `2 × 8 kv_heads × 128 dim × 2 B ×
  32 layers` ≈ **128 KiB**; um prefixo de 4k tokens ≈ **500 MiB**.
Implicação: partilha de KV é **LAN/NVMe**, só para prefixos repetidos e **mesmo
modelo+tokenizer+quant**. Nunca por omissão.

### 2.5 Speculative decoding distribuído

- Draft pequeno propõe N tokens; o verifier grande aceita/rejeita num passo.
- Se o draft estiver **no nó fraco** e o verify no forte, cada ronda custa
  1 RTT + verify. Só compensa em LAN e com taxa de aceitação alta.
Implicação: modo opcional, medido, com *fallback* automático para pedido
remoto quando a taxa de aceitação cai.

### 2.6 Transporte seguro P2P

- **Noise** (`XX`/`IK`): sem CA, autenticação mútua, forward secrecy, 1 RTT.
  É o que WireGuard, WhatsApp e libp2p usam para P2P.
- **QUIC**: 1 RTT, streams, migração de ligação, 0-RTT; `quic-noise` existe
  como especificação para P2P sem PKI.
- **TLS 1.3**: melhor se houver PKI/SPIFFE; tem PQ roadmap.
Implicação: `Noise IK` (chaves estáticas conhecidas) para trabalho; `Noise XX`
para o primeiro pareamento. QUIC fica para o dia em que houver NAT móvel.

### 2.7 Estado replicado

- **Delta-state CRDT**: mensagens pequenas, tolerante a perda (ao contrário de
  op-based que exige entrega causal exata).
- **Merkle-CRDT**: cada op é um nó content-addressed num DAG; dá causalidade,
  deduplicação, integridade e sync `O(log n)` via anti-entropy.
- **Automerge sync**: `heads` + bloom filter → 1 RTT típico; falsos positivos
  resolvidos com lista `need`.
Implicação: nsgdb passa a **log de ops + DAG**; o CRDT thin atual (LWW) fica
como materialized view.

### 2.8 Membership

- SWIM: carga constante por nó, deteção de falha por ping indireto.
- HyParView: vista parcial para gossip escalável.
- Plumtree: árvore epidémica + gossip de reparação.
Implicação: HB atual chega para 3–10 nós; acima disso, SWIM-lite.

### 2.9 WASM / wasmi

- `wasmi` é interpretador (não JIT), **determinístico**, com **fuel metering**
  estável — ideal para nós fracos e para reprodutibilidade.
- WASI Preview 2 + Component Model = interfaces tipadas (WIT) e capacidades
  explícitas; sandbox por construção.
Implicação: skills e "app swap" em wasmi; o host expõe apenas as capacidades
que o módulo declara (fs restrito, net allowlist, sem `exec`).

### 2.10 Frugalidade

- FrugalGPT/RouteLLM/cascatas: 50–98% de custo abaixo com qualidade próxima,
  roteando por dificuldade e escalando só quando o modelo barato falha.
Implicação: a frota deve ter **modelos por patamar** (0.5B/3B/8B/70B) e um
router que começa no barato local e escala para o peer forte.

### 2.11 TEE / atestação

- GPU TEE e atestação CPU↔GPU ainda imaturas, mas são o único caminho para
  "prova de execução" sem confiar no operador.
Implicação: **fase futura**, não bloqueia M6–M8. Marcar como `AWAITING_HW`.

---

## 3. Invariantes (não negociáveis, herdados do AGENTS.md)

1. **Identidade não é payload.** Nó = chave; `agent`/`caps` do JSON não contam.
2. **Fail-closed** em tudo: mensagem sem assinatura válida, contador repetido,
   nó fora da allowlist, kind desconhecido → descartar e registar.
3. **Sem root por rede.** Trabalho remoto nunca executa `nixos-rebuild`, nunca
   toca `/etc`, nunca arma o ring.
4. **Sem auto-join.** Pareamento com código curto (SAS) + allowlist.
5. **Quotas por peer.** tokens/s, bytes, jobs concorrentes, VRAM reservada.
6. **HITL para ações de frota** (mudar papel, revogar nó, distribuir skill).
7. **Nada de segredos no wire.** Chaves em `/var/lib/nosd/keys` (0600), nunca
   em logs; payloads de inferência não são persistidos por omissão.
8. **Honestidade ADR-0113.** Cada modo tem SLO declarado; se não mediu, é
   `residual`.

---

## 4. Arquitetura (camadas)

```
┌──────────────────────────────────────────────────────────────┐
│ L7 Governança: HITL, quotas, audit, kill switch, política    │
├──────────────────────────────────────────────────────────────┤
│ L6 Skills: wasmi components (WASI p2), content-addressed     │
├──────────────────────────────────────────────────────────────┤
│ L5 Cognição: router frugal, cascata, speculative, KV/prefix  │
├──────────────────────────────────────────────────────────────┤
│ L4 Trabalho: JobEnvelope, scheduler, deadline, idempotência  │
├──────────────────────────────────────────────────────────────┤
│ L3 Estado: Merkle-CRDT (nsgdb) + anti-entropy bloom          │
├──────────────────────────────────────────────────────────────┤
│ L2 Membership: SWIM-lite, HB assinado, allowlist, revogação  │
├──────────────────────────────────────────────────────────────┤
│ L1 Link: NoProto v2 + Noise IK (TCP) / QUIC opcional         │
├──────────────────────────────────────────────────────────────┤
│ L0 Identidade: Ed25519, node_id, keystore, (TPM opcional)    │
└──────────────────────────────────────────────────────────────┘
```

### L0 — Identidade

- Par `Ed25519` por nó, gerado no primeiro arranque (`/var/lib/nosd/keys/node.key`,
  0600, dono `nosd`), com `node.pub` publicável.
- `node_id_short: u32` (compatibilidade com o wire atual) **derivado** de
  `BLAKE3(pub)[0..4]`, não escolhido. `node_id_full: [u8;32]` no HB assinado.
- Rotação: chave nova assina a antiga ("continuity proof"); revogação propaga
  por CRDT assinado.
- (Fase 5) selar a chave num TPM quando existir; senão, ficheiro + Landlock.

### L1 — Link (NoProto v2)

Formato fixo, sem JSON, sem `serde` no caminho quente:

```
+--------+--------+---------+-----------+------------+-----------+
| magic  | ver u8 | type u8 | flags u16 | len u32 LE | nonce u64 |
+--------+--------+---------+-----------+------------+-----------+
| payload (len bytes)                                         |
+-------------------------------------------------------------+
| sig 64 B (Ed25519 sobre tudo acima + nonce do canal)        |
+-------------------------------------------------------------+
```

- `TaskType` (NoProto) cresce: `1=HB, 2=MCH, 3=ROLE, 4=FRAG, 5=FRACK,
  6=JOB_REQ, 7=JOB_RES, 8=LayaIntent, 9=CAP_ADS, 10=CRDT_DELTA,
  11=KVREF, 12=SKILL_PUSH, 13=REVOKE`.
- Transporte: **TCP + Noise IK** (`snow`), um stream por peer, com
  multiplexação lógica por `type`/`job_id`. QUIC quando houver NAT móvel.
- **Anti-replay**: contador monotónico por (peer, canal) + janela de 1024;
  repetido/atrasado = drop + audit.
- **Não** se assina HB em UDP broadcast com a chave de trabalho; o HB UDP é
  assinado mas **só serve para descoberta** (não autoriza trabalho).

### L2 — Membership

- HB assinado a cada 1 s (UDP unicast para a allowlist; broadcast só em lab).
- Estados: `Unknown → PinPending → Trusted → Stale → Revoked`.
- `Trusted` exige SAS confirmado (6 dígitos derivados de
  `BLAKE3(pub_a || pub_b)`) ou allowlist prévia.
- Deteção de falha: `direct ping` + `ping-req` a 2 vizinhos quando >3 nós.
- Eleição: capacidade **medida** (`tokens/s` de um micro-benchmark, VRAM livre,
  RTT) e não declarada; empate por `node_id_short`.

### L3 — Estado (nsgdb + CRDT)

- nsgdb passa a guardar `ops` (content-addressed, BLAKE3) + `heads`.
- Tipos: `LWWRegister` (fleet/role), `ORSet` (skills instaladas), `RGA/Counter`
  (métricas), `BlobRef` (modelos/KV/wasm).
- Anti-entropy a cada 5 s: `heads` + bloom filter (1%) → deltas; reparação com
  lista `need` (padrão Automerge).
- Materialized view: o `CrdtStore` atual continua a servir o HUD/health.

### L4 — Trabalho

```rust
struct JobEnvelope {
    job_id: Uuid7,          // ordenável por tempo
    kind: JobKind,          // Chat|Embed|Vision|Stt|Tts|Skill
    model_ref: BlobRef,     // hash do GGUF (e do tokenizer/quant)
    prompt_ref: BlobRef,    // payload em content-address (não inline)
    deadline_ms: u64,
    budget: Budget,         // tokens, ms, fuel wasm
    caps_required: CapBits, // avaliados pelo worker, não pelo cliente
    idempotency: [u8;16],
    reply_to: PeerId,
}
```
- `master` distribui; `worker` aceita/rejeita **antes** de ler o payload
  (quota/VRAM/modelo).
- Resultado: `JOB_RES { job_id, ok, tokens, usage, sig }`, assinado pelo worker.
  O master verifica assinatura e (opcional) hash do output.
- Retry: máx 2, com backoff; **idempotente** por `job_id` (o worker devolve o
  resultado em cache).
- Streaming de tokens: mensagens `JOB_TOK` com `seq`, mesma assinatura de canal.

### L5 — Cognição

| Modo | Onde usar | Requisitos | SLO honesto |
|---|---|---|---|
| **Remote prompt** (worker corre tudo) | WAN, modelos grandes | RTT < 200 ms | latência = rede + inferência do worker |
| **Pipeline PP** | LAN ≥ 1 Gbit, 2–4 nós | RTT < 1 ms | −20 a −50% tokens/s vs single-node; permite modelos que não cabem |
| **Expert offload** (MoE) | nó com RAM grande + GPU | LAN/NVMe | depende do hit-rate de experts |
| **Speculative** (draft local) | LAN, modelo forte remoto | RTT baixo, aceitação > 60% | ganho só acima do limiar; senão fallback |
| **KV/prefix reuse** | prefixos longos repetidos | mesmo modelo+tokenizer+quant | LAN; ~128 KiB/token |
| **Skill wasm** | funções frugais | wasmi + fuel | determinístico, sem rede por omissão |

Router frugal: `custo_estimado = tokens_previstos × preço_peer + RTT×peso`;
começa no modelo local mais barato; escala se `confidence < τ` ou se o validador
rejeitar (cascata). Registrar sempre `route.explain` para auditoria.

### L6 — Skills / app swap (wasmi)

- Módulo = `.wasm` content-addressed (`BLAKE3`), manifesto WIT com capacidades:
  `fs.read:/var/lib/nosd/skills/<id>`, `net.connect:allowlist`, `time.now`.
- Execução: `wasmi` com `fuel` (ex.: 50M) + limite de memória + timeout; sem
  threads, sem `exec`, sem `nix.store`.
- "App swap": a versão ativa de uma skill é uma **chave CRDT** (`skill/<nome>/active`
  → `BlobRef`). Trocar = 1 op assinada; todos os nós convergem; rollback = op
  anterior (não é delete).
- Distribuição: `SKILL_PUSH` (FRAG) ou pull por hash; verificação obrigatória.

### L7 — Governança

- `audit.log` (JSONL, rotativo) por nó: quem enviou o quê, decisão, quota.
- Kill switch por camada: `NOSD_MESH=off`, `NOSD_REMOTE_INFER=off`,
  `NOSD_KV_SHARE=off`, `NOSD_SKILLS=off`.
- Ações de frota exigem **HITL grant** (reutiliza o `confirm_id` do alpha.8).
- Relatório `fleet.health` honesto: modos ativos, quotas, nós `Revoked`.

---

## 5. "Telepatia" — canal não-textual (o que é e o que não é)

**O que é:** troca de *estado* em vez de linguagem.

| Mensagem | Conteúdo | Porque não é prompt |
|---|---|---|
| `LayaIntent` | braço de intenção, confiança, fnv do texto | 1 datagrama, sem tokens |
| `CAP_ADS` | modelos, VRAM, tokens/s medidos, RTT | capacidade, não pedido |
| `CRDT_DELTA` | ops de estado (frota, skills, memória) | merge semântico, não texto |
| `KVREF` | hash de prefixo + bloco KV | evita reprocessar 4k tokens |
| `EMB_DELTA` (fase 4) | vetor quantizado (int8) de embedding | similaridade, não frase |
| `JOB_*` | envelope binário + resultado | contrato, não conversa |

**O que não é:** não substitui raciocínio; não é "LLM a falar com LLM por
vetores mágicos"; não dispensa o modelo. É o equivalente a *RPC de estado*
entre processos — só que entre máquinas.

**Regra de ouro:** se a mensagem precisa de um LLM para ser interpretada, é
prompt e paga-se em tokens. Se pode ser aplicada por código (merge, verificação,
cache), é telepatia e paga-se em bytes.

---

## 6. Segurança: modelo de ameaça e mitigação

| Ameaça | Vetor | Mitigação | Teste |
|---|---|---|---|
| **MITM** | ARP/DNS na LAN | Noise IK (mútua) + pinning | `mitm_peer_rejected` |
| **Replay** | reenvio de HB/JOB | contador + janela + nonce | `replay_dropped` |
| **Spoof de HB** | anúncio de CAP falso | assinatura + allowlist + medida | `unsigned_hb_ignored` |
| **Sybil** | 1000 nós falsos | pareamento SAS + allowlist + quota por chave | `sybil_cap_enforced` |
| **Roubo de GPU** | jobs infinitos | quota tokens/s + deadline + budget | `quota_exceeded` |
| **Resultado adulterado** | worker mentiroso | assinatura + hash + (fase 5) TEE | `tampered_result_rejected` |
| **Prompt injection via peer** | peer envia "instruções" | payload é dado, nunca instrução; tools gated por CapGate local | `peer_text_not_instruction` |
| **Modelo trocado** | GGUF malicioso | hash no `BlobRef` + magic + tamanho | `wrong_model_hash` |
| **Wasm hostil** | skill maliciosa | fuel + memória + capacidades WIT + sem rede | `wasm_fuel_exhausted` |
| **DoS** | flood de FRAG | rate-limit por peer + reassembly cap | `frag_flood_dropped` |
| **Exfiltração** | skill lê segredos | caps de fs por skill + sem `node.key` | `skill_cannot_read_key` |
| **Equivocação** | master mente a dois nós | log assinado + CRDT DAG (auditável) | `equivocation_detected` |

Regra: **nenhum** destes testes pode depender de root; todos correm em 2 VMs.

---

## 7. Roteiro (fases, gates, evidência)

### Fase M6 — Identidade + Link (2–3 semanas-dev)
- `nos-ident` (Ed25519, keystore, node_id), `nos-link` (NoProto v2 + Noise IK).
- HB assinado; allowlist; contador anti-replay; `REVOKE`.
- **Gate:** `mesh-2vm-signed` — nó sem chave/fora da allowlist não entra; replay
  descartado; MITM (chave trocada) falha o handshake.

### Fase M7 — Trabalho remoto (3–4 semanas)
- `nos-job`: envelope, scheduler, quotas, idempotência, resultado assinado.
- `infer.remote` no CapGate + RPC `job.submit`/`job.poll` no `nosctl`.
- Implementação: worker corre `llama-server` **local**; master encaminha.
- **Gate:** 2 VMs, A sem modelo, B com tiny GGUF → `chat` em A devolve resposta
  de B, assinada; quota excedida recusada; resultado adulterado rejeitado.

### Fase M8 — Estado + KV (3–4 semanas)
- nsgdb como log de ops + DAG; delta-CRDT + bloom anti-entropy.
- `KVREF`/prefix sharing (opt-in, mesmo modelo).
- **Gate:** partição de rede → convergência após reconexão; KV reuse medido com
  ganho declarado (ou `residual` se não compensar).

### Fase M9 — Skills wasm + frugalidade (3 semanas)
- wasmi + WASI p2 (subset), content-addressed store, `skill/<n>/active` em CRDT.
- Router frugal/cascata + speculative opcional.
- **Gate:** skill distribuída e trocada em 2 nós (app swap atómico); fuel
  exhausted mata o módulo; cascata com `route.explain` auditável.

### Fase M10 — Metal (bloqueado por hardware)
- 3 máquinas heterogéneas (Intel iGPU, NVIDIA, AMD): PP LAN, expert offload,
  spec decode, medir tokens/s e latência por modo.
- **Gate:** `M4`/`M5` VERIFIED só com repetição ≥3 e artefacto.

### O que **não** se faz (não-objetivos)
- TP por rede; auto-join aberto; blockchain/token; "frota autónoma sem humano";
  inference em WASM; treino federado antes de servir estar sólido.

---

## 8. Contratos concretos (para não haver ambiguidade)

**`CAP_ADS`** (NoProto type 9, assinado):
```
models: [ {blob: [u8;32], arch: u16, quant: u8, ctx: u32} ]
vram_free_mb: u32, ram_free_mb: u32
tok_s: u16, rtt_us: u32, roles: bitflags
quota: {tok_per_s: u16, jobs: u8}
```

**`JOB_REQ`** (type 6): cabeçalho fixo + `prompt_ref` (hash), nunca o prompt
inline acima de 4 KiB (acima disso, blob + FRAG).

**`CRDT_DELTA`** (type 10): `[ {ns: u8, key: [u8;16], lamport: u64, writer: u32,
op: u8, payload}]` — merge idempotente; sem `payload` = tombstone.

**RPC local** (`nosctl`): `job.submit --peer <id> --prompt ... --deadline`,
`job.poll <id>`, `fleet.peers`, `fleet.trust <id> --sas 123456`,
`skill.install <wasm> --caps fs.read:...`, `skill.activate <nome> <hash>`.

---

## 9. Mudanças no repo

| Novo | Função |
|---|---|
| `crates/nos-ident` | chaves, node_id, assinatura/verificação, keystore |
| `crates/nos-link` | NoProto v2, Noise IK, anti-replay, framing |
| `crates/nos-job` | envelope, scheduler, quotas, idempotência |
| `crates/nos-crdt` | DAG + delta + bloom (extraído de `nos-mesh`) |
| `crates/nos-wasm-bus` | wasmi host, WIT, fuel, caps |
| `modules/skynet.nix` | opções (allowlist, quotas, kill switches) |
| `checks.mesh-2vm-*` | 6 testes negativos + 2 de trabalho |

Alterações: `nos-mesh` passa a usar `nos-ident`/`nos-link`; `nosd` ganha
`rpc/fleet.rs`; `nosctl` ganha os subcomandos acima.

---

## 10. Riscos e critérios de aborto

| Risco | Prob. | Impacto | Mitigação |
|---|---|---|---|
| Latência mata o ganho de PP | Alta | Médio | medir antes de prometer; fallback remote-prompt |
| Complexidade cripto | Média | Alto | `snow`/`ring` auditados; nada de cripto caseira |
| CRDT cresce sem controlo | Média | Médio | compaction + tombstone GC + cap por ns |
| Skills wasm viram bypass | Média | Alto | WIT mínimo, sem rede, fuel, revisão |
| Frota "telepática" virar moda sem ganho | Alta | Médio | só entra modo com SLO medido |
| Hardware heterogéneo | Certa | Alto | fases 6–9 correm em QEMU; 10 só com metal |

**Abortar/adiar** se: (a) PP em LAN < 10% ganho vs remote-prompt; (b) Noise +
framing > 5% do CPU de um job; (c) qualquer teste negativo ficar intermitente.

---

## 11. Referências

- llama.cpp RPC: `tools/rpc/README.md`, issues #22850, #23568; discussão #9136.
- PETALS: arXiv 2209.01188; DiLoCo: arXiv 2311.08105; OpenDiLoCo.
- Noise Protocol Framework: noiseprotocol.org; libp2p QUIC/Noise.
- Delta-state CRDT: arXiv 1803.02750; Merkle-CRDT: arXiv 2004.00107.
- Automerge sync (heads + bloom) — blog Automerge 2.0.
- SWIM (Cornell), HyParView (Leitão 2007), Plumtree.
- wasmi (fuel, determinismo), WASI Preview 2 / Component Model.
- FrugalGPT (TMLR 2024), RouteLLM, cascade routing (Dekoninck et al.).
- LMCache (arXiv 2510.09665), Mooncake (FAST'25).
- Speculative decoding (NVIDIA blog; SSD 2026).
- KTransformers (SOSP'25) — expert offload CPU/GPU.
- TEE/confidential inference — imaturo para GPU; AWAITING_HW.
