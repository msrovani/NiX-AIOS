# AGENTS.md — regras persistentes do NixAIOS

Este ficheiro é memória operacional para agentes/contribuidores. Leia antes de
mexer no código. O plano vivo está em [`docs/IMPROVEMENT_PLAN.md`](docs/IMPROVEMENT_PLAN.md).

## Norte

NixAIOS é a bancada NixOS/Linux do `neural-os-core` (ADR-0118), **não** um fork
do Ring0. O aceite primário é **M3 — Boot→JARBAS Jarvis-like**, seguido de
**M4 metal** e **M5 frota**. Ver [`docs/BOOT_TO_JARBAS.md`](docs/BOOT_TO_JARBAS.md)
e [`docs/REMIGRATION_PLAN_LEDGER.md`](docs/REMIGRATION_PLAN_LEDGER.md).

Versão corrente: **0.6.0-alpha.9** (`v0.6.0-alpha.9`). Volid ISO: **`NIXAIOS051`**.

## Regras não negociáveis

1. **Identidade não é payload.** `agent`, `caps`, `cap_gen` são legado e nunca
   autorizam. Unix = `SO_PEERCRED`; TCP/HTTP = token (`<data-dir>/ipc.token`).
2. **HITL é server-authored.** Nonce de uso único, TTL e kind. Um booleano
   `hitl_confirmed` do cliente **não** é HITL.
3. **Nenhum caminho root sem autenticação.** O ring Nix fica desarmado
   (`NOSD_NIX_RING` não setado) até S-04 + R-03/R-04 verdes.
4. **Fail-closed.** Ferramenta ausente, parse falhado, identidade ausente = deny.
5. **Nada de comandos nus em serviços NixOS.** Use caminho absoluto ou
   `systemd.services.<x>.path`.
6. **Sem seed de blobs na ISO** (GGUF/AgentJev/Piper). Runtime + HITL.
7. **Sem Ring0, X11, wasmtime path A, BitNet/Falcon3 nativo.**
8. **Honesty ADR-0113:** UI completed ≠ aceite; QEMU ≠ metal; `EngineAbsent` é
   honestidade, não capacidade. `VERIFIED` exige artefato + repetição.
9. **Teste no nível do bug.** PATH/systemd não se prova com unit test.
10. **Uma release, um objetivo.** Hardening não vira feature.

## Layout

```
packages/nosd/crates/    workspace Rust (nosd, nos-core, nos-hermes, ...)
modules/                 NixOS modules (nosd, cage, transmute, nix-ring)
images/iso.nix           ISO NIXAIOS051
scripts/                 build/smoke/gate/metal
docs/                    arquitetura, API, segurança, ADRs, ledger, plano
```

## Licoes que ja custaram caro (ler antes de mexer)

Resumo executavel; detalhe em
[`docs/memory/SESSION_FLEET_M6_M9.md`](docs/memory/SESSION_FLEET_M6_M9.md).

1. Flake usa a **arvore git**: `git add` antes de `nix build` (ficheiros novos).
2. `path` e `StartLimit*` sao atributos do **unit**, nunca de `serviceConfig`
   (o systemd ignora em silencio).
3. Unit com shebang `#!/usr/bin/env bash` precisa de `bash` no `path`.
4. Servico que corre como `nosd` **nao** pode `systemctl start` (polkit):
   usar `.path`/ficheiro gatilho, e nunca reiniciar o daemon que serve o pedido.
5. `/run/user` e tmpfs do logind e esconde o que la foi criado: usar
   `/run/<servico>/user`.
6. cage/wlroots so faz page-flip na VT activa: `chvt` no `ExecStartPre`; e
   `WAYLAND_DISPLAY` so no filho (no cage = modo aninhado).
7. `println!` em panico com EPIPE: tratar `BrokenPipe` (grep -q/head).
8. `#[tokio::main]` + `block_on` = panic: UI (Slint) fora do runtime.
9. Com identidade, o id do no **deriva** da chave (senao o pinning recusa).
10. `cargoExtraArgs` foi removido no nixpkgs 26.11 (silencioso): usar
    `cargoBuildFlags`.
11. Guardar artefactos no repo (`docs/evidence/`): `%TEMP%` e `C:\DEV\_logs`
    podem ser limpos.
12. Comandos com `|`/aspas via PowerShell -> `bash -lc` quebram: usar script.

## Comandos

```bash
cd packages/nosd
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check -p nos-jarbas --features slint-ui
```

Host TCP (Windows/lab):

```bash
cargo run -p nosd -- --foreground --tcp 127.0.0.1:7700 --data-dir ./data
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 --token-file ./data/ipc.token health
cargo run -p nosd --bin nosctl -- --tcp 127.0.0.1:7700 --token-file ./data/ipc.token chat "oi"
```

ISO/QEMU (WSL/Linux):

```bash
bash scripts/build-iso.sh
bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=/tmp/nixaios-qemu.log bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

## Adicionar um RPC

1. Método + caps em `nos-ipc::Request::required_cap` (e side-effect em
   `nos-hermes::opar_gate` se aplicável).
2. Handler em `packages/nosd/crates/nosd/src/rpc/<domínio>.rs`.
3. Subcomando em `nosctl.rs`.
4. Teste de contrato (`ipc_registry`) + teste de integração se tocar auth.
5. Se for root/apply, **não** religue o ring; adicione ao kill switch S-06.
6. Atualize ledger/CHANGELOG/docs.

## Estado conhecido (não fingir que está pronto)

- **S-04** HITL grant: **feito** (alpha.8) - `hitl_confirmed` ignorado; tudo por
  `confirm_id` server-authored; `nosctl consent <kind>`.
- **S-07** sanitização de `fetch_to_dir`: feito.
- **S-08/S-09** suite de seguranca + socket 0660: feito (alpha.6).
- **E-01/E-02/E-04** engine persistente/lock/GGUF: e2e VM com tiny GGUF PASS.
- **M-01** seg.bin fsync/cap: feito.
- **R-03/R-04** ring Nix: **decisao = desarmado** (armRing=false); helper root
  recusa pedidos sem NOSD_NIX_RING=1, target fixo, staging confinado, lock,
  timeout. So ligar com S-04 completo + ADR.
- **R-09** ISO 051/QEMU: OBSERVED (0.5.6).
- **R-05** NixOS VM test: feito (`checks.x86_64-linux.nixaios-vm`).
- **Q-03** CI Nix/VM: criado; validar no GitHub.
- **M3 Boot->JARBAS**: OBSERVED em QEMU (alpha.5: firstboot auto -> transmute
  -> hotswap -> cage/DRM -> HUD Slint; virtio-gpu + winit-software/pixman).
  GL/iGPU real, VT/teclado fisicos e metal = AWAITING_HW.
- **M5 lab**: check `mesh-2vm` (2 VMs, UDP 19000) OBSERVED: peers=1, TOFU
  pk-bound, Master/Worker. **Frota real e CRDT wire** continuam pendentes.
- **M6 identidade**: HB Ed25519 assinado + pinagem TOFU + allowlist +
  anti-replay; `mesh-2vm-signed` PASS. Sem Noise (confidencialidade) e sem
  revogacao ainda.
- **M8 estado**: CRDT (LWW+VV) sobre o link assinado do M7 (`kind=2`);
  `crdt.put/get/sync`; `mesh-2vm-crdt` PASS. Anti-entropy = estado completo
  (sem delta/Merkle), em claro, sem persistencia nsgdb.
- **S1 skills trust**: BLAKE3 (64 hex) + manifesto `skill/<n>/meta` +
  `skill.explain <focus>` com score 0-100 e `why` no CRDT. **Sem** assinatura
  de publisher/allowlist (S2) nem historico no score.
- **S2a publisher**: assinatura do manifesto (`name|focus|version|hash`) +
  allowlist `NOSD_SKILL_PUBLISHERS`; provenance `signed`. **Falta**: caps
  declaradas, historico, versao monotonica, push assinado, revogacao.
- **S2b caps/versao**: caps declaradas validadas (allowlist `aios_ai.infer`,
  `log`) e downgrade por `focus` recusado. Falta: historico no score, push
  assinado, revogacao e downgrade via HITL.
- **S2 skills (falta)**: assinatura de publisher + allowlist + caps declaradas
  + historico no score + versao monotonica. Desenho em
  `docs/SKILL_TRUST_AND_SELECTION.md`.
- **M9.1 SKILL_PUSH**: modulo viaja cifrado (`kind=3`) e so activa se o hash
  bater; 512 KiB/push (base64 no envelope); hash FNV, sem gossip/revogacao.
- **M8.1e log autenticado**: linha = json+tab+sig Ed25519 do no; replay recusa
  linha invalida/sem sig (com identidade). Sem anti-rollback; continua JSONL.
- **M8.1c compactacao**: runtime (60 ticks) + fsync do diretorio apos rename.
  Falta backend nsgdb (M8.1d) e log autenticado em repouso.
- **M8.1b durabilidade**: log `crdt.jsonl` (append + replay, fail-closed ao
  abrir). Nao e nsgdb ainda; sem compactacao/fsync por op; log nao autenticado.
- **M8.1 delta**: `crdt.sync` = pull por VV + push do delta (2 trocas cifradas).
  Sem bloom/Merkle; estado em memoria (sem nsgdb) e sem compactacao.
- **M6.1 Noise**: link de jobs/estado cifrado (XX + BIND Ed25519 contra a chave
  fixada). Sem modo plaintext. Sem revogacao/rotacao; HB UDP continua em claro.
- **M9 skills**: wasmi frugal (fuel 100k/heap 256 KiB/zero caps),
  `skill.install|list|run`, registo `skill/<n>/active` no CRDT. Local apenas
  (sem SKILL_PUSH); fnv como id, sem BLAKE3 nem quota.
- **M7 jobs**: `job.submit` + listener TCP `NOSD_JOB_BIND` com frames
  assinados; worker tem de ser peer fixado. Sem Noise/fila/quota por peer.
  Check `mesh-2vm-jobs` PASS. Ver `docs/SKYNET_DISTRIBUTED_PLAN.md`.
- **M4/M5**: AWAITING_HW.

## Evidência 0.6.0-alpha.9

`docs/RELEASE_0.6.0-alpha.9.md`, `docs/evidence/S2B_SKILL_CAPS_0.6.0-alpha.8.md`, `docs/evidence/S2A_SKILL_PUBLISHER_0.6.0-alpha.8.md`, `docs/evidence/S1_SKILL_TRUST_0.6.0-alpha.8.md`, `docs/evidence/M8.1e_CRDT_LOG_AUTH_0.6.0-alpha.8.md`, `docs/evidence/M8.1c_CRDT_COMPACT_0.6.0-alpha.8.md`, `docs/evidence/M8.1b_CRDT_LOG_0.6.0-alpha.8.md`, `docs/evidence/M8.1_DELTA_0.6.0-alpha.8.md`, `docs/evidence/M9.1_SKILL_PUSH_0.6.0-alpha.8.md`, `docs/evidence/M9_SKILLS_0.6.0-alpha.8.md`, `docs/evidence/M7_JOBS_0.6.0-alpha.8.md`, `docs/evidence/M6.1_NOISE_0.6.0-alpha.8.md`, `docs/evidence/M6_IDENTITY_0.6.0-alpha.8.md`, `docs/evidence/HITL_0.6.0-alpha.8.md`, `docs/RELEASE_0.6.0-alpha.7.md`, `docs/evidence/NIX_RING_0.6.0-alpha.7.md`, `docs/RELEASE_0.6.0-alpha.6.md`, `docs/evidence/SECURITY_S08_S09_0.6.0-alpha.6.md`, `docs/RELEASE_0.6.0-alpha.5.md`, `docs/evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md`, `docs/evidence/CAGE_SLINT_0.6.0-alpha.4.md`, `docs/evidence/VOICE_0.6.0-alpha.2.md`, `docs/evidence/VM_GGUF_0.6.0-alpha.2.md`, `docs/evidence/QEMU_0.6.0-alpha.2.md`.
