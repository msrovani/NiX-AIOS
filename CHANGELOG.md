# Changelog

Formato inspirado em [Keep a Changelog](https://keepachangelog.com/).  
Versionamento [SemVer](https://semver.org/).

## [0.6.0-alpha.3] — 2026-10-27 (Hot download + voz)

### Added

- **S-04 blobs:** `model.fetch`/`firmware.fetch` exigem grant single-use
  (`blob.fetch`/`firmware.fetch`) emitido por `ui.action`; replay volta a
  `await JARBAS HITL`. `nosctl fetch --confirm-id`; `nosctl ui-open
  --grant-kind`.
- **Voz CLI:** `nosctl voice-stt --wav-path` e `nosctl voice-tts [--no-play]`.
- **VM voz:** `voice-status Ready`; `voice-tts` WAV via espeak fallback
  (1.23s); `voice-stt` com `ggml-tiny.bin`+`jfk.wav` passou (manual, 23 min
  em TCG). Fixtures são de teste, não ISO.
- **VM accept-m3:** `scripts/accept-m3.sh` executado no VM.

### Evidence

- [`docs/evidence/VOICE_0.6.0-alpha.2.md`](docs/evidence/VOICE_0.6.0-alpha.2.md)
- `ipc_auth`: grant de blob + replay + `fetch` sem HITL.
- `nixaios-vm` PASS (GGUF/chat/unload/reload, accept-m3, voice-status/tts).

### Residual

- Cage/DRM, firstboot tty, S-08/S-09, R-03/R-04, metal.
- `voice-stt` real não entra no CI padrão (tempo TCG).

## [0.6.0-alpha.2] — 2026-10-27 (GGUF real no VM)

### Fixed

- **R-01 (crítico):** `path` estava dentro de `serviceConfig`; systemd ignorava
  (`Unknown key 'path'`) e `nosd` não via `llama-server`, `nmcli`, `nix`, etc.
  Movido para o atributo do serviço (`systemd.services.nosd.path`) e para os
  helpers `nosd-nix-apply`/`nosd-nix-rollback`.
- **Landlock:** allowlist RO para `/nix/store`, `/run/current-system`, `/etc`,
  `/usr`, `/lib`, `/lib64`, `/bin`, `/sbin`, `/run`, `/var/log`, `/dev`.
  Sem isto, subprocessos (`llama-server`, `nmcli`, `systemctl`) falhavam com
  `landlock Verified` (exec/read negado).
- **llama-server:** spawn com `--api-key` (aleatória, nunca logada) e
  `--no-webui`; health e infer enviam `Authorization: Bearer`; remove o aviso
  de CORS aberto.

### Added

- **VM GGUF real:** `checks.x86_64-linux.nixaios-vm` instala
  `stories15M-q4_0.gguf` (fixture, não ISO), valida `llama-server-http`,
  `chat`, `unload` e reload.
- `NOSD_LLAMA_CTX` opcional para `llama-server -c`.

### Evidence

- VM test: `llama-server spawned ... api_key=set`, `engine=llama-server-http`,
  `infer ... ok=true`, unload/reload, health após cada passo.
- Latência CPU/TGC ~38s no tiny model (não é SLO de produto).

## [0.6.0-alpha.1] — 2026-10-26 (Engine persistent)

### Added

- **E-01:** `llama-server` persistente como default quando há GGUF e o binário
  existe (`NOSD_LLAMA_SERVER_BIN`/PATH); health `/health` (30s), sem modelo →
  `cpu-echo`; `NOSD_LLAMA_SERVER` externo continua suportado; fallback
  `llama-cli`.
- **E-02:** `HotSwapController` usa `Arc<dyn InferenceBackend>` e não segura o
  lock durante inferência/subprocess; log `latency_ms`.
- **E-04:** validação de header GGUF (magic/versão/contadores) antes de attach.
- **M-01:** `seg.bin` com `sync_data` e cap de 16 MiB por registo.
- **J-01:** `scripts/accept-m3.sh` (health/route/chat/recall/voice-status/ui.card).

### Changed

- `llama_server_posture` distingue `managed+engine=llama-server-http`.
- Testes de env em `nos-hermes` serializados (ENV_LOCK); S2 pending com espera.

### Evidence

- `cargo fmt/clippy/test` verdes (3 runs); `nix build .#nosd`.
- VM test `.#checks.x86_64-linux.nixaios-vm` continua health+chat.

### Residual

- Chat com GGUF real/llama-server e2e, voz, cage/DRM e Wi-Fi não exercitados.
- S-04 completo (legado admin), S-08/S-09, metal M4/M5.

## [0.5.6] — 2026-10-26 (QEMU green)

### Fixed

- **nixpkgs 26.11:** `substituteAll` removido; `transmute-stage.nix` usa
  `replaceVars` + `runCommand`.
- `nosd-early.nix`: byte cp1252 inválido → UTF-8.
- CRLF em `.nix`/`.sh` quebrava scripts gerados (`$'\r'`); normalizado para LF.
- `images/iso.nix`: `console=ttyS0,115200n8` para QEMU headless.

### Evidence

- ISO 2.7 GiB, volid `NIXAIOS051`, SHA-256
  `26be2fb47a6e2a3b194dd9b5b1ef1a853d1450cc7b34a24dbafb00e5f09fc11b`.
- QEMU TCG: `Started Neural-OS Daemon`, `Finished NixAIOS early boot`,
  `Started JARBAS hub`, `Reached target Multi-User System`; gate MQEMU PASS.
- [`docs/evidence/QEMU_0.5.6.md`](docs/evidence/QEMU_0.5.6.md).

### Pending

- QEMU ≠ metal; chat/voz/cage/Wi-Fi não exercitados neste boot.
- Firstboot tty1 não dispara em ttyS0.
- S-04 completo, S-08/S-09, R-05 VM test, Q-03 CI.

## [0.5.5] — 2026-10-26 (HITL & Fetch)

### Security

- **S-04 (parcial):** HITL server-authored com grant de uso único, TTL 5 min e
  kind-bound, emitido por `ui.action` e devolvido como `confirm_id`.
  `self.heal`, `wifi.connect`, `nix.apply/confirm/build-vm` e `transmute.run`
  exigem grant; replay e kind errado são negados. `hitl_confirmed` permanece
  apenas como compatibilidade de `LocalAdmin` (root/token) e será removido na
  próxima fatia.
- **S-07:** `fetch_to_dir` aceita só basename (sem `../`, `/`, `\`), cap de
  8 GiB, `Content-Length`/stream cap, escrita `.partial` + rename, verificação
  de magic `GGUF` para `.gguf` e limpeza em falha.

### Added

- `HitlGrantStore` + testes (single-use, kind-bound).
- `nosd/tests/ipc_auth.rs`: fluxo `ui.open` → `ui.action` → `confirm_id` →
  `self.heal.evaluate`, replay negado, sem grant negado, ring desarmado.
- Testes de `safe_filename`, traversal, GGUF magic e parser HTTP.

### Changed

- `nosctl wifi-connect --confirm` e `nosctl nix-apply --confirm` (admin legado).
- Workspace/flake `0.5.5`.

### Pending (honesty)

- S-04 completo: remover `hitl_confirmed` do admin e wire `transmute`/UI cards.
- S-08/S-09: suíte de regressão dedicada + socket `0660`.
- R-09/QEMU, Q-03 CI/VM, R-03/R-04 ring, M4/M5.

## [0.5.4] — 2026-10-26 (Trust & Truth)

> 0.5.3 foi o nome de trabalho; o ship real é 0.5.4.

### Security

- **S-01/S-02:** identidade IPC derivada do transporte. Unix usa `SO_PEERCRED`;
  TCP/HTTP exige token. `agent`, `caps` e `cap_gen` do cliente deixam de autorizar.
  Perfil `LocalUser` (chat/read-only) e `Jarbas` (sem `NIX_WRITE`/spawn).
- **S-03:** `security.rotate` troca token TCP/HTTP + geração; tokens antigos são
  revogados. `nosctl security-rotate`.
- **S-06:** ring Nix (apply/confirm/build-vm/rollback) **desarmado por default**;
  `NOSD_NIX_RING=1` é opt-in explícito após HITL nonce + VM test.
- **HTTP lab:** **off por default** (`NOSD_ORB_LAB=""`); o ISO só liga no modo
  cinematic. `/rpc` exige token, `application/json`, same-origin e limites de
  corpo/tempo; removido `Access-Control-Allow-Origin: *` e o fallback silencioso
  para `health` em JSON inválido.
- Token em `<data-dir>/ipc.token` (`0600` em Unix); nunca logado. TCP não-loopback
  exige `--ipc-token` explícito.

### Changed

- `nosctl` e `nos-jarbas` aceitam `--token`/`--token-file` (env
  `NOSD_IPC_TOKEN`/`NOSD_IPC_TOKEN_FILE`). Quickstart TCP atualizado.
- `nosctl` lê o token também no socket Unix (token-first → `LocalAdmin`);
  JARBAS continua sem token → `Jarbas`. No ISO, `nos-transmute` usa `sudo`
  para o primeiroboot/transmute (user `nixos` não é admin do daemon).
- Testes de S1/memória usam funções puras (sem `std::env`), removendo flakiness.
- Workspace `cargo fmt` limpo e `clippy -D warnings` limpo.

### Fixed

- Slint HUD compila com `--features slint-ui` (padding CSS shorthand inválido e
  `var_os().as_deref() == Some("1")`).
- 5 testes de integração novos em `nosd/tests/ipc_auth.rs` (sem token, agent
  legado, token válido, rotação, ring desarmado; Unix token-first).
- **R-01:** `services.nosd.path` declara networkmanager/nix/util-linux/gnutar/
  alsa-utils/pipewire/llama-cpp/whisper-cpp/espeak-ng; helper root declara
  `nixos-rebuild`. `nosd` entra no grupo `systemd-journal`.
- **R-02:** `run_syntax_check` é fail-closed: sem `nix-instantiate` não há
  `syntax_ok:true`; `nix.propose` falha honestamente.

### Notes

- Ring Nix continua **desarmado** até S-04 (HITL por nonce) e R-03/R-04.
- S-04, S-07 e R-09 (ISO/QEMU) são as próximas fatias de 0.5.5/0.6.0.

## [0.5.2] — 2026-10-25

### Added

- `HermesEngine::boot(BootConfig)` + `tick_mesh_hints` / `tick_soul_mirror`.
- RPC fatiado: `crates/nosd/src/rpc/{health,model,chat,nix_wifi,ui_orb,voice_transmute,factory_store,http_lab}.rs`.
- Scripts externos: `modules/transmute/{firstboot,hotswap-aios}.sh`.
- `self.heal.evaluate` RPC; teste IPC registry; `nos-core::fnv1a64`.
- [`docs/RELEASE_0.5.2.md`](docs/RELEASE_0.5.2.md); canónico `scripts/build-iso.sh`.

### Changed

- Workspace / flake → `0.5.2` (volid ISO permanece `NIXAIOS051`).
- `NOSD_AUTOLEARN` default **off**; ModelHub só via `state.hermes.hub`.
- Scripts s5xx/FAT lab → `scripts/archive/`.

### Removed

- Stub IPC `mesh.skillsync` (sem wire útil).

## [0.5.1] — 2026-10-25

### Added

- Documentação moderna: hub [`docs/README.md`](docs/README.md), ARCHITECTURE, GETTING_STARTED, DEVELOPMENT, API, OPERATIONS, SECURITY, GLOSSARY, ROADMAP.
- Raiz: CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, LICENSE (AGPL-3.0), CHANGELOG.
- [`docs/RELEASE_0.5.1.md`](docs/RELEASE_0.5.1.md); ISO volid `NIXAIOS051`.

### Changed

- Workspace / flake version → `0.5.1`.
- Empacota e publica em Git o fecho técnico 0.5.0 (nunca tagged isolado).

### Notes

Funcionalidade metal/Linux-first documentada em [`docs/RELEASE_0.5.0.md`](docs/RELEASE_0.5.0.md).

## [0.5.0] — 2026-10-25 (código; ship Git = 0.5.1)

### Fixed

- Metal pós-Q5: `resolve_models_dir` **Linux-first** (evita EACCES quando FAT/Landlock desalinhados).
- Landlock allowlist inclui `/mnt/nixaios-log` (defesa se FAT opt-in).
- Firstboot: `save_file`/`set_step` falham de forma visível; erros `transmute` impressos no tty.
- Cage/ISO: permissões `1777` em `/var/lib/nosd` (sem `chmod 750` agressivo no pre-start).
- Paths de voz/locale sem preferência FAT.

### Added

- `NIXAIOS_USE_FAT_PERSIST=1` + probe de escrita.
- CRDT thin LWW + version vector (`nos-mesh::crdt`).
- `SoulMirrorSnapshot` + EventBus `SOUL_MIRROR`.
- `llama_server_posture()` honesty (S2.2).
- `evaluate_self_heal` CapGate Deny-by-default.
- `scripts/build-iso-050.sh`, `scripts/smoke-transmute-paths.sh`.

### Security

- CapGate server-authored (I.1); S1.2 no-seed; guard ISO.

### Honesty / residual

- Boot metal / lab 2-nó / CUDA VERIFIED — fora do aceite.

## [0.1.0] — 2026-10-21 … 2026-10-25

Scaffold NixAIOS (ADR-0118), remigração s529. Ver ledger de remigração.
