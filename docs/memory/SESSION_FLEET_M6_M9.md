# Sessão frota M6→M9 + S1/S2 — lições aprendidas

**Data:** 2026-10-27  
**Resultado:** 0.6.0-alpha.9 publicada (tag `v0.6.0-alpha.9`), ISO
`ff1f3f53…`, gate MQEMU PASS.  
**Objetivo desta nota:** não repetir os erros na próxima sessão.

---

## 1. Nix / build

| Lição | Consequência |
|---|---|
| O flake usa a **árvore git** (`git+file:`); ficheiros novos têm de estar `git add`ados antes de `nix build` | perdi um ciclo com `ident.rs` (build falhou a dizer que o módulo não existia) |
| `cargoExtraArgs` **removido** no nixpkgs 26.11 e ignorado em silêncio | a ISO saía sem `slint-ui` (HUD em slog); usar `cargoBuildFlags` |
| `src` do buildRustPackage copia `target/` do host (GBs, read-only) | `nix build` falha com `Read-only file system`; filtrar com `builtins.path { filter = ... }` |
| KVM é *system feature* do builder | `nixosTest` pode falhar com `missing system features {kvm}`; o teste corre na mesma em TCG |

## 2. systemd (a fonte de bugs mais silenciosa)

| Lição | Sintoma real |
|---|---|
| `path` **tem** de ser `systemd.services.<x>.path` | dentro de `serviceConfig` o systemd escreve `Unknown key 'path'` e ignora → `llama-server`/`nmcli`/`systemctl` “não existem” |
| `StartLimit*` também é nível de **unit** | em `serviceConfig` é ignorado → não há anti-loop |
| `#!/usr/bin/env bash` precisa de `bash` no `path` | `env: 'bash': Arquivo ou diretório inexistente` (status 127) |
| Serviço que corre como `nosd` **não** pode `systemctl start` | polkit devolve “interactive authentication required” (exit 1); usar `.path`/ficheiro gatilho |
| Reiniciar o daemon que serve o pedido mata a resposta | `transmute` dava `nosctl exit=1`; só reiniciar se o health estiver offline |
| `DeviceAllow` implica `DevicePolicy=closed` | `StandardInput=tty` falha com `208/STDIN` sem `char-tty` |

## 3. Runtime gráfico (cage / Wayland)

| Lição | Sintoma |
|---|---|
| `/run/user` é **tmpfs do logind** no 1º login e esconde o que lá estava | `mkdir: cannot create directory '/run/user': Permission denied` → usar `/run/<svc>/user` |
| wlroots/seatd só faz page-flip na **VT activa** | 1º arranque automático ficava preto até `systemctl restart jarbas-cage`; `chvt` no `ExecStartPre` (root) |
| `WAYLAND_DISPLAY` no ambiente do **cage** faz o cage correr aninhado | `Creating wayland backend / Could not connect to remote display`; passar só ao filho |
| `LIBGL_ALWAYS_SOFTWARE=1` quebra o EGL do wlroots | retry do HUD falhava; usar `SLINT_BACKEND=winit-software` + `WLR_RENDERER=pixman` |
| Slint/winit escolhe X11 se `WINIT_UNIX_BACKEND` não for forçado | Xwayland + glamor falha e o HUD morre |

## 4. Rust / tokio / CLI

| Lição | Sintoma |
|---|---|
| `#[tokio::main]` + `Runtime::block_on` = panic | `Cannot start a runtime from within a runtime` (Slint + `rpc_blocking`) |
| `println!` entra em **pânico** com `EPIPE` | `nosctl chat \| grep -q` dava exit 101; tratar `BrokenPipe` como sucesso |
| `thread_local!` de runtime reutiliza o runtime por thread | evita 5 runtimes por tick no poll |
| Com identidade, o id do nó **tem** de ser derivado da chave | pinning recusava `IdMismatch` quando o id vinha do `--node-id` |
| `ed25519-compact` precisa da feature `random` para gerar chaves | `KeyPair::generate` não existia |
| `ed25519-compact` **não** converte Ed25519→X25519 | Noise IK exigiria matemática de curva; XX + BIND Ed25519 resolve sem PKI |

## 5. Segurança (o que se aprendeu a fazer)

- **Nunca aceitar booleano do cliente como HITL** (S-04): o grant é
  server-authored, single-use, kind-bound. Nem root converte `hitl_confirmed`.
- **Id de conteúdo**: FNV serve para dedup, **não** para integridade. BLAKE3 é o
  mínimo para commitment; assinatura de publisher prova **autoria**.
- **Caps declaradas** têm de ser validadas contra allowlist fechada; uma skill
  que declara `fs.read` não pode ser instalada.
- **Versão monotónica**: downgrade silencioso é ataque; recusar sem HITL.
- **Log em repouso**: assinar cada linha (Ed25519 do nó) impede edição casual;
  não impede quem tem a chave (TPM é `AWAITING_HW`).

## 6. Ferramentas / ambiente (Windows + WSL)

| Lição | Consequência |
|---|---|
| `C:\Users\...\Temp` e até `C:\DEV\_logs` podem ser limpos | guardar artefactos que importam **no repo** (`docs/evidence/`) |
| WSL encheu o C: (store 162 GB) e passou a dar `I/O error` / `Read-only file system` | `nix-collect-garbage -d` (151 GiB) + limpar `%TEMP%` e caches; não apagar dados do utilizador |
| `wsl --shutdown` a meio de builds pode corromper estado | esperar pelo fim do build antes de desligar |
| Passar `|`/`%`/aspas por PowerShell para `bash -lc` mangla o comando | escrever **scripts** em ficheiro, não pipelines inline |
| Python helpers com literais `bytes` não aceitam não-ASCII | usar `str` + `.encode('utf-8')` |
| Scripts que fazem múltiplas edições misturando `str`/`bytes` podem crashar a meio | deixar ficheiros parcialmente editados (aconteceu na alpha.9); preferir escrever o ficheiro inteiro |

## 7. Processo que funcionou

1. Uma fatia = um commit + uma evidência em `docs/evidence/`.
2. Check de VM **antes** de escrever a release (e re-correr quando o binário muda).
3. Release: bump → build ISO → boot + gate → hash na nota → tag → push.
4. Nunca declarar “feito” sem o teste negativo correspondente; residuais sempre
   escritos na nota.

## 8. Estado à saída desta sessão

- **Feito:** M6, M6.1 (Noise), M7, M8, M8.1, M8.1b/c/e, M9, M9.1, S1, S2a, S2b,
  alpha.9 publicada.
- **Falta:** S2c (histórico/push assinado/revogação/downgrade HITL), M8.1d
  (`nsgdb` + anti-rollback), HB de descoberta em claro, frota real, M4/M5 metal.
- **Próximo passo recomendado:** S2c → M8.1d → metal.
