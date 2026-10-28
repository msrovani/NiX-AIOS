# Evidence ? S1 skills trust (BLAKE3 + manifesto + escolha)

**Data:** 2026-10-27  
**Estado:** implementado e provado (`ipc_auth` 15 PASS).

## O que mudou

| Antes (M9.1) | Agora (S1) |
|---|---|
| hash `fnv1a64` (dedup) | **BLAKE3 hex (64)** como commitment de conte?do |
| sem metadados | manifesto `skill/<nome>/meta` = `{name,focus,version,caps,hash,bytes,provenance,published_at_ms}` |
| sem escolha | `skill.explain <focus>` pontua candidatas e escreve `focus/<f>/active` + `why` |
| push sem manifesto | push grava meta com `provenance="peer"`; install local `"local"` |

**Score (0?100), determin?stico:** proveni?ncia (30/18/10) + caps m?nimas
(20?4?n) + vers?o (40 se for a m?xima do foco, sen?o 20) + tamanho (10 se for
o menor) ? desempate por vers?o e depois nome. O `why` fica no CRDT, por isso a
decis?o ? replic?vel e audit?vel.

`skill.explain`/`select` exigem `MEMORY` (escrevem estado).

## Provas (`ipc_auth`)

| Teste | Resultado |
|---|---|
| `skill_focus_selection_prefers_newer_and_uses_blake3` | hash com **64** hex; `sum2@2.0.0` vence `sum1@1.0.0`; `focus/sum/active` = `sum2@2.0.0`; `why.version > 0` |
| `skill_install_run_and_list` / `fuel` / `infer_denied` | continuam verdes |

## Honestidade / residual (S2)

- **Sem assinatura de publicador**: `provenance` distingue `local` vs `peer`,
  mas um peer malicioso continua a poder empurrar um m?dulo que **passa** o
  hash (o hash s? prova integridade, n?o autoria).
- **Sem allowlist de publishers** nem `caps` declaradas (o campo existe e est?
  vazio; o sandbox continua com zero capacidades).
- **Sem hist?rico local** (sucesso/falha) no score ? sempre 0 hoje.
- **Sem vers?o monot?nica**: instalar uma vers?o mais antiga com `focus`
  diferente ainda ? poss?vel; o explicador escolhe a maior.
- Continua 512 KiB por push e FNV como `mesh_knowledge` (n?o ? hash de skill).
