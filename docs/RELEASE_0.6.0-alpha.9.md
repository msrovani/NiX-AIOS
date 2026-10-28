# NixAIOS 0.6.0-alpha.9 — Frota (identidade → skills)

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.9  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.9`  
**Base:** 0.6.0-alpha.8

## O que entra

Esta release junta as fatias de frota implementadas desde alpha.8. **Nenhuma
delas foi declarada “frota pronta”** — cada uma tem evidência própria e
residuais escritos.

### Identidade e canal

- **M6** — `nos-mesh::ident`: Ed25519, id **derivado** da chave, HB assinado
  (`seq` + `KEY` + `SIG`), pinagem TOFU, anti-replay, allowlist
  (`NOSD_MESH_ALLOWLIST`), `NOSD_MESH_REQUIRE_SIG=1`.
- **M6.1** — link de jobs/estado em **Noise XX** (X25519 descartáveis) +
  `BIND` Ed25519 ligado ao `handshake_hash` e verificado contra a chave fixada.
  Sem modo plaintext (não há downgrade).

### Trabalho e estado

- **M7** — `job.submit` sobre TCP autenticado, frame `NJOB` assinado, worker
  tem de ser peer fixado, resultado assinado, limites (prompt 8 KiB, corpo
  1 MiB, deadline 0.1–120 s).
- **M8** — CRDT (LWW + version vector) sobre o mesmo link (`kind=2`),
  `crdt.put/get/sync`.
- **M8.1** — `crdt.sync` por **delta** (pull por VV + push do que falta).
- **M8.1b/c** — log append-only `crdt.jsonl` com replay, compactação no
  arranque **e em runtime** (60 ticks), `fsync` do diretório, tolerância a
  linha truncada.
- **M8.1e** — cada linha do log **assinada** (Ed25519 do nó); replay recusa
  linha inválida ou sem assinatura.

### Skills (wasmi)

- **M9** — `skill.install/list/run` com sandbox frugal (fuel 100k, heap
  256 KiB, zero capacidades), registo em CRDT.
- **M9.1** — `skill.push` cifrado com **verificação de hash antes de activar**.
- **S1** — hash **BLAKE3** (64 hex), manifesto `skill/<n>/meta`
  (focus/version/caps/provenance), `skill.explain <focus>` com score 0–100 e
  `why` auditável no CRDT.
- **S2a** — assinatura de publisher (`name|focus|version|hash`) + allowlist
  `NOSD_SKILL_PUBLISHERS`.
- **S2b** — `caps` declaradas validadas (só `aios_ai.infer`/`log`) e
  **downgrade recusado** por `focus`.

## Evidência

| Fase | Artefacto |
|---|---|
| M6 | `docs/evidence/M6_IDENTITY_0.6.0-alpha.8.md` |
| M6.1 | `docs/evidence/M6.1_NOISE_0.6.0-alpha.8.md` |
| M7 | `docs/evidence/M7_JOBS_0.6.0-alpha.8.md` |
| M8 / M8.1 | `docs/evidence/M8_CRDT_*`, `M8.1_DELTA_*` |
| M8.1b/c/e | `M8.1b_CRDT_LOG_*`, `M8.1c_CRDT_COMPACT_*`, `M8.1e_CRDT_LOG_AUTH_*` |
| M9 / M9.1 | `M9_SKILLS_*`, `M9.1_SKILL_PUSH_*` |
| S1 / S2a / S2b | `S1_SKILL_TRUST_*`, `S2A_SKILL_PUBLISHER_*`, `S2B_SKILL_CAPS_*` |

Checks de frota: `mesh-2vm`, `mesh-2vm-signed`, `mesh-2vm-jobs`,
`mesh-2vm-crdt`, `mesh-2vm-skills` (todos PASS).

## Residuais (declarados, não esquecidos)

- **S2c**: histórico (sucesso/falha) no score, push assinado, revogação por
  CRDT/HITL, downgrade via HITL.
- **M8.1d**: backend `nsgdb` (hoje JSONL próprio) e anti-rollback do log.
- **Noise**: HB de descoberta continua assinado mas **em claro**; sem
  revogação/rotação de chaves.
- **Frota real** (>2 nós, Wi-Fi, perda de pacotes, NAT) e **M4/M5 metal**
  continuam `AWAITING_HW`.
- **Skills**: caps declarativas (o host continua zero-caps); sem TEE, o
  resultado de um job não é verificável além da assinatura.
