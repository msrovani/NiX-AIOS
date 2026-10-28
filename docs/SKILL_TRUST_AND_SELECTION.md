# Skills: confiança, qualidade e escolha pelo Hermes

**Estado:** requisito + desenho. Hoje (M9/M9.1) **não** cumpre “evitar malware”:
o id é FNV-1a64, não há assinatura de autor, não há manifesto nem scoring.
**Objectivo:** publicar/activar skills com garantias verificáveis e deixar o
Hermes escolher, entre candidatas com o mesmo foco, a que está melhor.

---

## 1. Estado real (para não fingir)

| Propriedade | Hoje |
|---|---|
| Integridade do módulo | `fnv1a64` (dedup, **não** commitment) |
| Autoria | nenhuma (só a identidade do canal no push) |
| Manifesto | implícito (nome + hash no CRDT) |
| Capacidades | zero por omissão (bom), mas não declaradas |
| Sandbox | wasmi: fuel 100k, heap 256 KiB, 1 memória/tabela |
| Versão / downgrade | não existe |
| Escolha entre 2 apps do mesmo foco | **não existe** |

Conclusão: um módulo malicioso hoje **entra** se quem instala tiver `FS`
(LocalAdmin), e não há como distinguir “app bom” de “app com o mesmo nome”.

---

## 2. Modelo de ameaças (skills)

| # | Ameaça | Hoje | Mitigação exigida |
|---|---|---|---|
| T1 | Wasm adulterado em trânsito | Noise+Ed25519 do frame (bom) | + hash forte (BLAKE3) no manifesto |
| T2 | Publicador impostor | nenhum | chave de publicador + assinatura do manifesto |
| T3 | Typosquatting (mesmo nome/foco) | nenhum | `publisher` + `focus` no manifesto; escolha auditada |
| T4 | Escalada de capacidades | caps = NONE | manifesto declara caps; host só expõe o declarado; instalar exige aprovação se caps > política |
| T5 | Abuso de recursos | fuel/heap fixos | orçamento por skill (fuel/tempo/mem) + quota por peer |
| T6 | Downgrade para versão vulnerável | nenhum | versão monotónica por `focus`; downgrade só com HITL |
| T7 | Supply chain (dependência trocada) | nenhum | lock do módulo por hash; `skill.lock` no CRDT |
| T8 | Execução de efeitos | sem imports úteis | lista fechada de imports (`aios_ai::infer` + `log`), sem fs/net/exec |
| T9 | Resultado envenenado | assinatura do envelope | resultado assinado (M7) + `provenance` no log |
| T10 | Reputação falsa | nenhum | reputação **local** (sucesso/falha observados), nunca global auto-reportada |

---

## 3. Formato de pacote v2

```jsonc
// skill manifest (assinado; vive em skill/<nome>/<versão>/meta)
{
  "name": "summarize",
  "focus": "text.summarize",        // agrupamento para escolha
  "version": "1.2.0",               // semver, monotónica por focus
  "entry": "run",                   // export wasm
  "caps": ["aios_ai.infer"],        // declaração; host valida
  "wasm_blake3": "…64 hex…",
  "wasm_bytes": 18432,
  "publisher": "…pubkey ed25519 (hex)…",
  "published_at_ms": 1761…,
  "sig": "…ed25519(sha256(manifest sem sig))…"
}
```

Regras de instalação (**fail-closed, por ordem**):

1. `wasm_blake3` do corpo **tem** de bater.
2. Assinatura do manifesto válida com `publisher`.
3. `publisher` em `trusted_publishers` (allowlist) **ou** igual ao peer que
   empurrou (`provenance=peer`) **ou** decisão HITL explícita.
4. `caps ⊆ caps_grants` (política do nó); qualquer cap extra → recusa.
5. Validação no wasmi + `start` sem efeitos + fuel de validação.
6. Versão `>=` a activa no `focus` (senão exige HITL e regista `downgrade`).
7. Só então: grava por hash, `skill/<nome>/<versão>` e actualiza o índice.

Proveniência registada: `local-admin` | `peer:<node_id>` | `signed:<publisher>`.

---

## 4. Escolha entre candidatas do mesmo foco (Hermes)

Chave de índice: `focus/<focus>/candidates = ORSet<nome@versão>`.
`focus/<focus>/active = nome@versão` + `focus/<focus>/why = {score, breakdown}`.

**Score (0–100), determinístico e auditável:**

| Critério | Peso | Como mede |
|---|---|---|
| Proveniência | 30 | `signed` (30) > `peer` (18) > `local-admin` (10) |
| Capacidade mínima | 20 | `20 - 4×n_caps_extra` (menos é melhor) |
| Histórico local | 20 | `20 × sucesso/(sucesso+falha)` (mín. 5 execuções, senão 8) |
| Eficiência | 10 | `10 × min(1, fuel_medido_alvo/fuel_medido)` |
| Versão | 10 | semver normalizada no `focus` (mais recente = mais) |
| Tamanho | 5 | menor é melhor (dentro do mesmo foco) |
| Idade | 5 | publicação recente (mas < 7 dias = penalização anti-pressa) |

Desempate: `version` (maior), depois `publisher` (ordem estável), depois nome.

**Fluxo:**
1. `skill.publish` adiciona candidato (com as regras do §3).
2. Em tick (ou `skill.select <focus>`), o Hermes pontua todos e escolhe.
3. A escolha é escrita no CRDT com `why` — reprodutível e auditável
   (`skill.explain <focus>` devolve o breakdown).
4. HITL pode fazer override: `skill.pin <focus> <nome@versao>` (uma op CRDT).
5. Se a activa falhar 3× seguidas, volta à segunda melhor e regista.

**Anti-patterns:** reputação global auto-reportada; “mais downloads” como sinal;
escolher pelo tamanho apenas; actualizar automaticamente sem verificação de
assinatura.

---

## 5. Testes (negativos primeiro)

| Teste | Espera |
|---|---|
| `wasm_bytes` alterado após assinar | recusa (`hash`) |
| manifesto com sig de outra chave | recusa (`sig`) |
| publisher fora da allowlist sem HITL | recusa (`untrusted`) |
| `caps` com `fs.read` | recusa (`cap_escalation`) |
| versão inferior à activa | recusa sem HITL (`downgrade`) |
| duas candidatas do mesmo foco | escolhe a de maior score; `why` explica |
| candidata melhor publicada depois | próxima selecção troca; log regista |
| skill activa falha 3× | fallback para a segunda; estado no CRDT |
| módulo com loop infinito | fuel exhausted (já coberto) |

---

## 6. Fases

- **S1 (próximo):** BLAKE3 + manifesto + `focus` + versão + `skill.explain`;
  sem chaves de publicador. Já permite escolha auditável e bloqueia
  typosquatting por `focus`.
- **S2:** chaves de publicador + assinatura do manifesto + allowlist
  (`trusted_publishers`), proveniência `signed` no score.
- **S3:** histórico local (sucesso/falha), orçamento por skill, fallback
  automático e override por HITL.

**Critério para chamar “seguro”:** S2 verde + os 9 testes negativos em CI, com
BLAKE3 como commitment e nenhum caminho que active módulo sem assinatura
válida. Até lá, a frase correcta é: *“sandbox frugal, sem garantia de
proveniência”*.
