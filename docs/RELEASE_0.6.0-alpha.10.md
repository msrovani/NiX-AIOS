# NixAIOS 0.6.0-alpha.10 — Autoria de skills + CRDT em nsgdb

**Volid ISO:** `NIXAIOS051`
**Workspace:** `packages/nosd` 0.6.0-alpha.10
**Data:** 2026-10-27
**Tag:** `v0.6.0-alpha.10`
**Base:** 0.6.0-alpha.9

## O que entra

Esta release fecha duas dividas declaradas na alpha.9: **S2c** (autoria e
revogacao de skills) e **M8.1d** (anti-rollback e backend nsgdb do log do CRDT).
Nao ha feature nova fora destas duas fatias.

### S2c — autoria e confianca de skills

- **Historico no score:** `skill/<n>/stats` no CRDT; com >=5 execucoes o score
  passa a pesar `20·ok/(ok+fail)` em vez de ficar conservador.
- **Autoria persistida:** publisher/signature/provenance ficam no manifesto
  (`skill/<n>/meta`), nao so no pedido de install.
- **Push assinado:** o emissor assina `nome|focus|versao|hash`; o destino
  verifica a assinatura **antes** de gravar. Prova e2e em VM.
- **Revogacao:** `skill/revoked/<publisher_hex>` no CRDT; a assinatura deixa de
  valer **mesmo sendo valida** (install e push), fail-closed.
- **Downgrade via HITL:** versao inferior a maxima do `focus` exige grant
  server-authored `skill.downgrade`; sem grant continua recusado.
- **CLI:** `nosctl skill-sign`, `nosctl skill-revoke`, flags
  `--focus/--version/--publisher/--sig/--confirm-id` no `skill-install`.

### M8.1d — log do CRDT em nsgdb

- **Anti-rollback:** ancora (`seg.anchor`/`crdt.anchor`, `<bytes> <linhas>`)
  ao lado do log; gravada ao abrir, ao compactar (antes do rename) e a cada 32
  registos. Um log em disco mais pequeno do que a ancora **impede o arranque**
  (fail-closed, com a mensagem a indicar o ficheiro a apagar).
- **Backend nsgdb:** o log passa a `data_dir/nsgdb/crdt/seg.bin` — segmento
  length-prefixed com cabecalho `NSGC`, registos `u32 len + payload`
  (`json\tbase64(sig)`), no mesmo espirito do backend de memoria. **Nao** e o
  motor neural-sgdb completo (sem indice/BQ/semantico).
- **Migracao one-shot:** no arranque, se existir `crdt.jsonl` legado sem
  segmento, o estado e reescrito no segmento e o legado fica em
  `crdt.jsonl.migrated`. `NOSD_CRDT_BACKEND=jsonl` mantem o formato antigo.

### CI

- Workflow com os 5 checks de frota (um job por check) e `--option
  system-features "kvm nixos-test ..."` no job de VM.
- **`scripts/ci-local.sh`**: espelho local do workflow, com `--fast` e frota
  **sequencial** (`--max-jobs 1`; 5 checks em paralelo = OOM em 7 GB).
- **Pendente:** a primeira execucao real no GitHub (sem `gh` autenticado neste
  ambiente). O codigo esta pronto; isto e prova externa, nao codigo.

## ISO

`nix build .#iso` ->
`/nix/store/gm542dq0kbwzha7lc1s04hikhdxvrns7-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso` ->
SHA-256 `98218b99ad3812e0ceef52b53e27e500644dd1df91df60bff7cf7aa09c32f115`.
Gate `MQEMU` **PASS** (`fails=0`, QEMU/TCG OBSERVED). Detalhe e serial:
`docs/evidence/QEMU_0.6.0-alpha.10.md`.

## Evidencia

| Fase | Artefacto |
|---|---|
| S2c (historico, autoria, push, revogacao, downgrade, CLI) | `docs/evidence/S2C_*`, `docs/evidence/CLI_SKILL_AUTH_0.6.0-alpha.9.md` |
| M8.1d anti-rollback | `docs/evidence/M8.1D_CRDT_ANCHOR_0.6.0-alpha.9.md` |
| M8.1d backend nsgdb | `docs/evidence/M8.1D_NSGDB_BACKEND_0.6.0-alpha.9.md` |
| ISO/QEMU | `docs/evidence/QEMU_0.6.0-alpha.10.md` |

Checks de frota: `mesh-2vm`, `mesh-2vm-signed`, `mesh-2vm-jobs`,
`mesh-2vm-crdt` (com `NSGC` nos dois nos), `mesh-2vm-skills` (com a cadeia
assinada) — todos PASS.

## Residuais (declarados, nao esquecidos)

- **CI no GitHub**: primeira execucao externa pendente (Q-03).
- **nsgdb**: e um segmento local; falta indice/consulta (replay completo ao
  abrir), migracao inversa e o motor neural-sgdb real (BQ/semantico).
- **Ancora** na mesma pasta do log: protege contra repor o log sozinho, nao
  contra quem escreve no data-dir (precisa TPM/contador confiavel).
- **S2d**: chaves de publicador separadas/delegacao, auditoria de revogacao e
  de grants, expiracao de revogacao.
- **Frota real** (>2 nos com netem/kill/particao) e **HB em claro** continuam
  em aberto.
- **M4/M5 metal**: `AWAITING_HW`. QEMU nao e metal; `EngineAbsent` continua a
  ser honestidade.
