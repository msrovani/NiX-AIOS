# NixAIOS 0.6.0-alpha.6 — S-08/S-09 (endurecimento do IPC)

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.6  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.6`  
**Base:** 0.6.0-alpha.5

## Objetivo

Fechar S-09 (permissões do socket Unix) e alargar S-08 (regressões de
segurança) **sem** novas funcionalidades: é uma release de hardening.

## S-09 — socket IPC `0660`

`serve_unix` faz `chmod 0660` imediatamente após o `bind` e **falha o
arranque** se não conseguir (`S-09: chmod 0660 ...`). Antes o modo vinha do
`umask` (tipicamente `0755`).

- Dono/grupo: `nosd:nosd`.
- Só o próprio `nosd` e membros do grupo `nosd` ligam ao socket.
- A autorização continua a ser `SO_PEERCRED` + caps (o modo é defesa em
  profundidade, não substitui auth).

## S-08 — regressões

Novos testes (além dos 8 existentes de identidade/token/grant):

| Teste | O que fixa |
|---|---|
| `ipc_auth::unix_socket_is_0660` | S-09: `stat` do socket == `0660` |
| `ipc_auth::unknown_method_is_denied_not_ignored` | fail-closed para método inexistente |
| `ipc_auth::admin_method_requires_token` | `security.rotate` sem token TCP negado, token intacto |
| `nos-ipc::rejects_oversized_frame` | frame com tamanho `u32::MAX` → `TooLarge` |
| `nos-ipc::unknown_method_is_deny_unknown` | `required_cap` devolve `DENY_UNKNOWN` |

Contagem: `nos-ipc` 3 testes, `ipc_auth` 10–11 (o teste de socket é
`cfg(unix)`), workspace verde.

## Verificação

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
nix build .#iso                      # buildRustPackage corre os testes em Linux
NIXAIOS_QEMU_LOG=... bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

ISO: SHA-256 `592a95d053b31ffaf2dd3cab02fa18937ab9217fc51dc23299e0fd366a6ace1c`;
`stat` no guest: `660 nosd:nosd`; gate MQEMU PASS; VM test PASS.
Evidencia: `docs/evidence/SECURITY_S08_S09_0.6.0-alpha.6.md`.

## Honestidade

- QEMU/VM ≠ metal. O socket `0660` foi provado por teste + `stat` em Linux,
  não por auditoria de um atacante real.
- `NOSD_NIX_RING` continua desarmado; R-03/R-04 seguem pendentes.
- Restam S-04 completo (admin legacy `hitl_confirmed`), CI no GitHub e M4/M5.
