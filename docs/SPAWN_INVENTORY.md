# Inventário de spawns — `nosd` (D1, 2026-10-29)

Regra (AGENTS.md lição 5 + padrão `PROCESS_SPAWN_ALLOWLIST` do oh-my-hermes):

1. **Serviço NixOS não herda o `PATH` de login.** O binário tem de estar em
   `systemd.services.nosd.path` (atributo do *unit*) **ou** ser um caminho
   absoluto resolvido por allowlist.
2. **O `PATH` não decide** o binário de um caminho privilegiado: `nvidia-smi`
   passou a usar `nos-core::proc` (candidatos absolutos + `NOSD_NVIDIA_SMI`).
3. Um binário novo no código é uma linha nova nesta tabela — no mesmo commit.

Auditoria: `grep -rn "Command::new(" packages/nosd/crates --include=*.rs`.

## Dentro do daemon (`nosd` / crates)

| Binário | Onde | Resolução | Estado |
|---|---|---|---|
| `nvidia-smi` | `nos-observe::energy`, `nos-cortex::capacity` | **allowlist absoluta** (`nos-core::proc`): `/run/current-system/sw/bin/nvidia-smi`, `/usr/bin`, `/usr/local/bin`, Windows | **D1 ✔** |
| `journalctl` | `nos-observe` | `systemd` no `path` do unit | **D1 ✔** (antes: vazio em NixOS) |
| `timeout` | `nos-voice` | `coreutils` no `path` | **D1 ✔** (antes: faltava) |
| `nmcli` | `wifi_nm`, `transmute`, `nos-observe`, `window_broker` | `networkmanager` no `path` | ✔ |
| `systemctl` | `actuators`, `transmute`, `nix_managed` | `systemd` no `path` | ✔ (polkit impede o `nosd` de arrancar units — lição 4) |
| `nix-instantiate` | `nix_managed` | `nix` no `path` | ✔ |
| `mountpoint` | `transmute` | `util-linux` no `path` | ✔ |
| `tar` | `transmute` | `gnutar` no `path` | ✔ |
| `wpctl`, `pw-play` | `actuators`, `nos-voice` | `pipewire` no `path` | ✔ |
| `arecord`, `aplay` | `nos-voice` | `alsa-utils` no `path` | ✔ |
| `espeak` | `nos-voice` | `espeak-ng` no `path` | ✔ |
| whisper CLI | `nos-voice` | `NOSD_WHISPER_CLI` (absoluto) ou `whisper-cli` no `path` | ✔ |
| `llama-server` / `llama-cli` | `nos-cortex` | `NOSD_LLAMA_SERVER_BIN`/`NOSD_LLAMA_CLI` ou `llama-cpp` no `path` | ✔ |

## Fora do daemon (sessão do utilizador — `nos-jarbas`)

| Binário | Onde | Nota |
|---|---|---|
| `xdg-open` / `cmd /c start` | `nos-jarbas::slint_app` | abrir o orb lab no browser; corre como utilizador, não como serviço |
| `chromium` / `cage` | unit `jarbas-cage` | caminho absoluto do store nos `ExecStart` |

## Dívida conhecida

1. Os binários "✔" ainda são **nomes nus** resolvidos pelo `path` do unit (não
   por caminho absoluto no código). Funciona, mas o acoplamento é invisível:
   renomear/remover um pacote do `path` quebra em runtime, não no build.
   Proposta (V9): gerar as constantes de caminho a partir do `pkgs` no build
   (`option_env!`) ou migrar para `nos-core::proc` por binário.
2. `NOSD_WHISPER_CLI`/`NOSD_LLAMA_CLI` aceitam caminho relativo — quem os define
   é o operador; validar `is_absolute()` no arranque seria mais estrito.
3. Este inventário é um **snapshot**; o teste é o `grep` no CI (a criar).
