# AGENTS.md — regras persistentes do NixAIOS

Este ficheiro é memória operacional para agentes/contribuidores. Leia antes de
mexer no código. O plano vivo está em [`docs/IMPROVEMENT_PLAN.md`](docs/IMPROVEMENT_PLAN.md).

## Norte

NixAIOS é a bancada NixOS/Linux do `neural-os-core` (ADR-0118), **não** um fork
do Ring0. O aceite primário é **M3 — Boot→JARBAS Jarvis-like**, seguido de
**M4 metal** e **M5 frota**. Ver [`docs/BOOT_TO_JARBAS.md`](docs/BOOT_TO_JARBAS.md)
e [`docs/REMIGRATION_PLAN_LEDGER.md`](docs/REMIGRATION_PLAN_LEDGER.md).

Versão corrente: **0.5.6** (`v0.5.6`). Volid ISO: **`NIXAIOS051`**.

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

- **S-04** HITL grant: parcial (admin legacy `hitl_confirmed` ainda existe).
- **S-07** sanitização de `fetch_to_dir`: feito.
- **S-08/S-09** suíte de segurança + socket 0660: pendente.
- **R-03/R-04** `ai-managed` import + helper root: ring desarmado.
- **R-09** ISO 051/QEMU: OBSERVED (0.5.6).
- **R-05** NixOS VM test: feito (`checks.x86_64-linux.nixaios-vm`).
- **Q-03** CI Nix/VM: criado; validar no GitHub.
- **M4/M5**: AWAITING_HW.

## Evidência 0.5.6

`docs/RELEASE_0.5.6.md`, `docs/RELEASE_0.5.5.md`, `docs/evidence/QEMU_0.5.6.md` e `docs/memory/SESSION_0.5.4_TRUST.md`.
