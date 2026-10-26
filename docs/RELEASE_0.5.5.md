# NixAIOS 0.5.5 — HITL & Fetch

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.5.5  
**Data:** 2026-10-26  
**Tag:** `v0.5.5`  
**Base:** 0.5.4 (Trust & Truth)

## O que muda

### S-04 — HITL server-authored (parcial)

- `ui.action` (Submit de janela HITL) emite um **grant**:
  `confirm_id` + `confirm_expires_ms`, single-use, kind-bound, TTL 5 min.
- Ações exigem grant: `self.heal.evaluate`, `wifi.connect`,
  `nix.apply`/`nix.confirm`/`nix.build_vm`, `transmute.run`,
  `systemctl.scoped`.
- Replay, kind errado e grant expirado são negados.
- `hitl_confirmed` ainda funciona **apenas** para `LocalAdmin` (root/token) como
  compatibilidade; JARBAS/LocalUser não usa booleano.
- O ring Nix continua **desarmado** (`NOSD_NIX_RING` opt-in).

### S-07 — Fetch endurecido

- `fetch_to_dir` aceita apenas basename: rejeita `../`, `/`, `\`, absoluto,
  `.` e `..`.
- Cap de 8 GiB por download (`Content-Length` + stream).
- Escrita `.partial` + `sync_all` + rename.
- `.gguf` só é aceite com magic `GGUF`; ficheiro inválido é removido.
- Limpeza de parcial em erro/tamanho excedido.

## Evidência

```bash
cd packages/nosd
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

- `nosd/tests/ipc_auth.rs`: 7 testes no host (o Unix roda em Linux/CI).
- `nos-cortex`: testes de `safe_filename`, traversal, GGUF magic.
- `nosd` lib: testes de grant single-use/kind e parser HTTP.

## Breaking / operação

- `ui.action` agora devolve `confirm_id`; clientes HITL devem propagá-lo para a
  ação destrutiva.
- `nosctl wifi-connect --confirm` e `nosctl nix-apply --confirm` (admin legado).
- `NOSD_NIX_RING=1` continua proibido até S-04 completo + R-03/R-04.

## Residual

- S-04 completo (remover bool do admin; wire UI cards de nix/transmute).
- S-08/S-09 (suíte dedicada + socket 0660).
- R-09/QEMU, Q-03 CI/VM, R-03/R-04 ring, M4/M5.
