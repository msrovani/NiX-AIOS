# NixAIOS 0.5.4 — Release notes (Trust & Truth)

**Volid ISO:** `NIXAIOS051` (inalterado; SemVer patch/minor no workspace)  
**Workspace:** `packages/nosd` 0.5.4  
**Data:** 2026-10-26  
**Tag:** `v0.5.4`  
**Tema:** fechar a fronteira de confiança do IPC e parar o caminho root acidental.

## O que é esta release

Uma release de **hardening**, não de features. O objetivo foi transformar o
CapGate/HITL de “encenação” em uma fronteira de transporte e remover o caminho
root que existia por acidente.

### Segurança

- **Identidade derivada do transporte** (`S-01`): Unix usa `SO_PEERCRED`
  (uid/gid); TCP/HTTP exige token de processo.
- **Payload não autoriza** (`S-02`): `agent`, `caps` e `cap_gen` são legado e
  ignorados. Sem identidade → `Unknown` (deny).
- **Token-first + rotação** (`S-03`): `security.rotate` troca token + geração;
  tokens antigos são revogados. Token em `<data-dir>/ipc.token` (`0600` em Unix),
  nunca logado. TCP não-loopback exige `--ipc-token` explícito.
- **HTTP lab fechado** (`S-05`): **off por default**; o ISO só liga no modo
  cinematic. `/rpc` exige token, `application/json`, same-origin (quando há
  `Origin`), corpo ≤ 1 MiB e timeout de 5 s. Sem `Access-Control-Allow-Origin: *`
  e sem fallback silencioso para `health` em JSON inválido.
- **Ring Nix desarmado** (`S-06`): `nix.apply`, `nix.confirm`, `nix.build_vm` e
  `nix.rollback` devolvem `NIX_RING_DISABLED` até `NOSD_NIX_RING=1` (opt-in
  explícito, após S-04 + R-03/R-04).
- **Perfis**: `LocalAdmin` (root/token), `Jarbas` (uid do daemon; sem
  `NIX_WRITE`/spawn), `LocalUser` (chat/read-only), `Unknown` (deny).

### Runtime / NixOS

- **R-01:** `services.nosd.path` declara `networkmanager`, `nix`, `util-linux`,
  `gnutar`, `alsa-utils`, `pipewire`, `llama-cpp`, `whisper-cpp`, `espeak-ng`.
  O helper root declara `nixos-rebuild`. `nosd` entra no grupo `systemd-journal`.
- **R-02:** `run_syntax_check` é fail-closed; sem `nix-instantiate` não existe
  `syntax_ok:true`.
- Primeiro boot/hotswap usam `nosctl health` no socket (não dependem mais do orb
  lab HTTP).
- `nos-transmute` usa `sudo` no ISO (o user `nixos` não é admin do daemon).

### Engenharia

- Testes determinísticos de S1/memória (sem `std::env` em paralelo).
- `cargo fmt --check` e `clippy --all-features -D warnings` limpos.
- Slint HUD compila com `--features slint-ui` (padding CSS shorthand e
  `var_os().as_deref()` corrigidos).
- 5 testes de integração novos em `nosd/tests/ipc_auth.rs` (TCP sem token, agent
  legado, token válido, rotação, ring desarmado; Unix token-first).

## Evidência (host Windows)

```bash
cd packages/nosd
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check -p nos-jarbas --features slint-ui
```

Resultado: **verde** (31 binários de teste; 5 testes de integração `ipc_auth` no
Windows; o teste Unix roda em Linux/CI).

PoC manual antes/depois:

| Cenário | 0.5.2 | 0.5.4 |
|---|---|---|
| Request sem `agent`/token | admin (`nosctl`) | `Unknown`/deny |
| `agent:"hermes"` sem token | FULL | deny |
| `nix.apply` com `hitl_confirmed:true` | armava request root | `NIX_RING_DISABLED` |
| `POST /rpc` com `Origin` externo | 200 + CORS `*` | 403 |
| JSON inválido em `/rpc` | virava `health` | 400 |
| Token antigo após rotação | n/a | deny |

## Breaking / upgrade

- **TCP agora exige token.** Use `--token-file <data-dir>/ipc.token` (ou
  `NOSD_IPC_TOKEN`/`NOSD_IPC_TOKEN_FILE`). `nosctl` tenta os caminhos
  `/run/nosd/ipc.token`, `/var/lib/nosd/ipc.token` e `./data/ipc.token`.
- `agent`/`caps`/`cap_gen` continuam no wire por compatibilidade, mas são
  ignorados.
- HTTP lab off por default; para o modo cinematic, `NOSD_ORB_LAB=127.0.0.1:7701`.
- Para armar o ring Nix: `NOSD_NIX_RING=1` **somente** após S-04/R-03/R-04.

## Residual honesto (fora desta release)

| Item | Estado |
|---|---|
| S-04 HITL por nonce (wifi/transmute/systemctl/self-heal) | pendente |
| S-07 fetch path sanitization + size/magic | pendente |
| S-08/S-09 suíte de segurança + socket 0660 | pendente |
| R-03/R-04 `ai-managed` import + helper root endurecido | ring desarmado |
| R-05/R-09 `nixosTest` + ISO/QEMU 051 | não executado neste host |
| Q-03 CI Nix/VM | não criado |
| D-03 `.gitattributes`/LF baseline | adiado (working tree normalizado) |
| Metal M4 / frota M5 | AWAITING_HW |

**Não** marcar M4/M5 como VERIFIED. **Não** religar o ring sem nonce. **Não**
embutir blobs na ISO.

## Build

```bash
bash scripts/build-iso.sh        # volid NIXAIOS051
bash scripts/guard-iso-no-s1-blob.sh /path/to.iso
```

## Próxima fatia

S-04 (HITL nonce) + S-07 (fetch) + S-08 (regressão) → depois R-09/QEMU + CI/VM →
depois 0.6 (`llama-server` persistente, voz, Wi-Fi, memória).
