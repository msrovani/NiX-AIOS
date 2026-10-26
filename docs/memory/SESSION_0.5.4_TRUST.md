# Sessão — 0.5.4 Trust & Truth

**Data:** 2026-10-26  
**Workspace:** `packages/nosd` 0.5.4  
**Volid:** `NIXAIOS051`  
**Plano:** [`../IMPROVEMENT_PLAN.md`](../IMPROVEMENT_PLAN.md)  
**Release:** [`../RELEASE_0.5.4.md`](../RELEASE_0.5.4.md)

## Contexto

A análise profunda do repo mostrou três coisas:

1. o CapGate “server-authored” confiava no campo `agent` do JSON; `agent`
   ausente virava `nosctl` (admin) e `agent:"hermes"` virava `FULL`;
2. o HTTP lab chamava `dispatch` direto, com CORS `*`, sem token e sem validar
   JSON (fallback para `health`);
3. o ring Nix armava um request root, mas `ai-managed.nix` não era importado e
   `nixos-rebuild` não estava no PATH do helper — um “root path” inerte por
   acidente, a uma linha de virar RCE.

Além disso, o serviço `nosd` não tinha `PATH` declarado; `nmcli`, `nix`,
`llama`, `whisper`, `tar`, etc. não eram encontrados no ISO.

## O que foi feito (0.5.4)

- Identidade por transporte: `SO_PEERCRED` (Unix) + token (TCP/HTTP).
- `agent`/`caps`/`cap_gen` ignorados.
- `security.rotate` + revogação de token.
- HTTP lab off por default; `/rpc` com token, JSON, same-origin, limites.
- Ring Nix desarmado (`NOSD_NIX_RING` opt-in).
- `services.nosd.path` + helper root com `nixos-rebuild`.
- `run_syntax_check` fail-closed.
- Testes S1/memória determinísticos; Slint compila.
- 5 testes de integração `ipc_auth`.

## Lições (memorizar)

1. **Identidade não é payload.** `agent`, `caps`, `cap_gen` são hints/legado.
   O transporte decide. Default deny.
2. **HITL booleano é teatro.** Precisa de nonce emitido pelo servidor, TTL,
   single-use e kind. S-04 ainda está pendente — não fingir que HITL fechou.
3. **Caminho root é uma linha.** Se um arquivo de config é copiado e
   `nixos-rebuild` roda como root, assuma RCE se o gatilho não é autenticado.
   Desarme o ring até o HITL por nonce existir.
4. **Teste no ambiente do produto.** `cargo test` no host não prova PATH de
   systemd. Serviços NixOS não herdam o PATH de login; declare `path`.
5. **Ferramenta ausente é falha, não “structural OK”.** Fail-closed.
6. **Testes que mutam `std::env` em paralelo são flaky.** Injete config ou
   serialise; 3/10 falhas foram reproduzidas.
7. **`cargo test` verde não é ISO verde.** QEMU/metal são evidência separada.
8. **Slint feature também é código.** `padding: 14px 20px` não é válido; o
   feature build estava quebrado.
9. **O lab HTTP é superfície de ataque.** Off por default; token; sem CORS `*`.
10. **Line endings importam.** Scripts CRLF quebram `bash` no Linux. O baseline
    LF + `.gitattributes` deve ser um commit separado.

## Evidência

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check -p nos-jarbas --features slint-ui
```

Antes: request sem `agent` → `nix.propose` ok; `nix.apply` armava request root;
`POST /rpc` com Origin externo → 200 + CORS.  
Depois: sem token → `Unknown`; `nix.apply` → `NIX_RING_DISABLED`; `/rpc` 401/403/
415/400; rotação revoga token antigo.

## O que NÃO foi feito

- S-04 nonce (wifi/transmute/systemctl/self-heal).
- S-07 sanitização do fetch.
- S-08/S-09 suíte de segurança + socket 0660.
- R-03/R-04 import `ai-managed` + helper root endurecido.
- R-05/R-09 VM test / ISO 051 / QEMU.
- Q-03 CI Nix/VM.
- D-03 `.gitattributes` + normalização definitiva.
- M4 metal / M5 frota.

## Próximo

S-04 + S-07 + S-08; depois R-09/QEMU + CI/VM; depois 0.6.
