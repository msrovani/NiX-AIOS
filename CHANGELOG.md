# Changelog

Formato inspirado em [Keep a Changelog](https://keepachangelog.com/).  
Versionamento [SemVer](https://semver.org/).

## [Unreleased]

- **Inferência/VRAM/voz/fábrica — plano `docs/PLAN_INFERENCE_VRAM_VOZ_FABRICA.md`
  (2026-11-06)**: sete fases implementadas e publicadas em `main` (`8f08cbd`,
  `d7b7884`, `575095a`, `7a62319`, `9ddfac7`, `c3ce2a4`, `d175b0c`; docs
  `b54a5f9`/`86e09b2`), gate host com **1037** testes, e ISO `iso-hw`
  reconstruída de `main` (`dbe9d0e5…`) para a prova QEMU.
  - F1: o `nosd` deixa de escrever `NOSD_LLAMA_NGL` (**quatro** sítios: boot,
    tick, `observe.snapshot` e transmute) e o cap térmico passa a `AtomicU32` do
    processo, aplicado só a `-ngl` explícito; `SwapError::Boot` acaba com o
    fallback silencioso para `llama-cli`.
  - F2: `fit_model` com `basis: vram|spill|cpu` (RAM só paga VRAM com
    `allow_spill`), `need_estimate`/`kv_est_mb`, `hub.evict`, `vram_free_mb` por
    placa, `register_path` com `conflict`/`candidates`.
  - F3: `ureq::Error::Status` → `SwapError::Http{code,body}`; retry só 400/422.
  - F4 voz (modelo do idioma, base antes de tiny, um backend por WAV, `aplay -l`);
    F5 fábrica (`nix build` pelo flake, `write` recusa dir preenchido,
    `Lang::detect`); F6 wasm (`max_pages` aplicado); F7 energia (RAPL soma todos
    os `intel-rapl:N`).
  - Prova QEMU/TCG (`docs/evidence/QEMU_INFERENCE_2026-11-06.md`): `nosd`
    `active`, `llama-server` sem `-ngl` e **uma** cópia; gate MQEMU `fails=0`
    com `SKIP` no `nosd` porque o auto-login em `ttyS0` cega o log de série
    (lição 62 do `AGENTS.md`). Metal **AWAITING_HW**.

- **Derrotas convertidas no host (2026-11-05)** — sem metal, três causas provadas:
  - `piper` **nunca carregava**: o tarball tem `PT_INTERP=/lib64/ld-linux-x86-64.so.2`
    (inexistente no NixOS) e a derivação tinha `dontFixup` → `execve` devolve ENOENT
    e `voice.tts` reportava `io: No such file or directory` com `tts_ready: true`.
    `piper-standalone` passa a `patchelf --set-interpreter` do store + wrapper com
    `LD_LIBRARY_PATH`; no host `--help` rc=0 e síntese real (170 260 B, 22 050 Hz).
  - **Chromium cinematic**: `Unable to get gpu adapter` era o probe do **modelo
    on-device** (`services/on_device_model/ml/gpu_blocklist.cc`), não o compositor.
    O render morria por **Vulkan com `--ozone-platform=wayland`**; o wrapper passa a
    `--disable-features=Vulkan,…OnDeviceModel…` e desliga a telefonia
    (`--disable-background-networking`; o log tinha GCM com `PHONE_REGISTRATION_ERROR`).
  - **HUD**: `04 VISÃO` sobe para o topo da coluna direita e a 720p passa a feed
    empilhado a toda a largura (168×126 → 297×185); artefacto
    `docs/evidence/artifacts/orb_hud_vision_right_720p.png`.

- **Correcções da colheita metal de 2026-11-03** (`docs/evidence/hw/2026-11-03_2346Z`):
  - `voice.stt` deixa de devolver `ok:true` com `text:""` — transcript vazio é
    erro explícito (`nos_voice::transcript_or_error`), a mesma doutrina de
    `percent:null`.
  - TTS: ficheiro temporário **vazio** (piper falhado) deixou de passar por
    bom; `wav_ready()` (＞44 B) dispara o fallback espeak e o erro final nomeia
    o caminho em vez de `io: No such file or directory`.
  - `-ngl` fixo removido do caminho `--fit`: `services.nosdEngine.nGpuLayers`
    default **0** (o fit decide) e `hw-cuda` deixa de forçar `NOSD_LLAMA_NGL=99`.
    Com 99, um 4B Q6 em 6 GiB parcialmente ocupados morre em `cudaMalloc OOM`
    (`nosd-engine-s2.service: Failed with result 'exit-code'`).
  - Probe M3b: A2 passa a medir o **socket** do PipeWire (as ferramentas são
    reportadas à parte) e A4 grava a **16 kHz mono**, o formato que o whisper
    aceita — o A2/A4 anteriores mediam o PATH e a taxa default do ALSA.
  - `scripts/extract-usb-logs.py`: extrai o bundle do pendrive `NIXAIOSLOG`
    (FAT32 lida no dispositivo cru, sem montar) para `logs/`, com filtro de
    `wifi.conf`/`firstboot.conf` e WAV, e `MANIFEST.txt` com sha256.
  - Lições **53–57** no `AGENTS.md` (pendrive cru; `is_file()` não valida
    conteúdo; `ok:true` vazio; `-ngl` anula `--fit`; PATH≠serviço e default≠
    formato) e memória da sessão em
    `docs/memory/SESSION_20261105_USB_HARVEST_FIXES.md`.
  - **`nvidia-smi` SIGSEGV — causa raiz provada no host**: o fault `0x401488`
    é `.dynamic(0x4013a0) + 0xe8` = `DT_DEBUG.d_ptr`, escrito pelo `ld.so`
    num `PT_LOAD` que o patchelf deixou só `PF_R` (`error 7` = escrita em
    página presente/user read-only); corrigido carrega sob glibc 2.44
    (original rc=139 → corrigido rc=0). O metal crashava porque o **perfil**
    servia o binário por corrigir: `buildEnv { ignoreCollisions = true }`
    resolve por `meta.priority` e o `nvidia_x11` tem 4 contra 5 do fix.
    `lib.hiPrio` nas duas imagens CUDA + check **comportamental**
    `nvidia-smi-loads` (corre o ELF sob o `ld.so` do alvo, não só as flags).
    Lições 58–59; memória
    `docs/memory/SESSION_20261105_NVIDIA_SMI_SIGSEGV.md`.

## [0.7.0-alpha.6] — HUD C1–C8 (correcções das fotos de metal) + pendrive FAT 1 GiB

- **HUD `orb_lab` C1–C6** (commits `2fa3e61` código + `01fd4f4`/`e60e930`
  evidência): C1 estado de rede derive do **link real** (`nos-observe::
  NetLinkState` `Associated|NoLink|Down`, `link_state()`), não de iff-up;
  C2 energia com **badge de scope** (`CPU|GPU|FULL|EST|—`) — a mentira
  silenciosa "MED só-GPU" morre; C4 painel **`02e VOZ`** separado do REDE
  + botão `PROPOR SUSPEND` completo com tag `TIER HITL`; C4 drawer de
  lições (DÍVIDA clicável → `openLessonsDrawer`, servindo
  `nix.open_lessons` com `hint_for()` em `nos-core::nix_domain`); C5
  painel de prioridades visíveis no topo 180°; C6 coluna esquerda = **25vw
  exacto** (kiosk + narrow).
- **Rebuild `iso-hw-cuda`** com HUD embutido + pendrive novo:
  ISO `/nix/store/nlbyyyix40jb0i7c4gw90saj92ba4f6f-…`, sha256
  `5747c666…8a41`; `.img` 9 397 338 112 B (8,75 GiB), sha256
  `09c4e76f…f194`, FAT32 **1 GiB** `NIXAIOSLOG` (`FAT_MB=1024` no
  `pack-hw-usb.sh`). S1 Astra Q4_K_M + S2 4B Q6_K + STT/piper provados
  por squashfs (409 matches). Commit docs: `9e3bfd3`.
- Evidência: `docs/evidence/ISO_HW_CUDA_S1ASTRA_2026-11-04.md`;
  memória: `docs/memory/SESSION_20261104_HUD_C1C6.md`.
- Metal (boot no pendrive novo, FAT 1 GiB legível no Win11)
  **AWAITING_HW**.

## [0.7.0-alpha.5] — neural-sgdb MemoryGovernor + pin sync

- **S1 swap (2026-11-04, in-tree)**: o GGUF embutido na `iso-hw`
  passa a ser o **Qwen3.5-0.8B editado pela Astra** (Weight-Space
  Programming, MIT CSAIL/RikiyaT 2026-11-02): subtracção de uma direcção
  de loop em down_proj das camadas 15/19 — 131 072 pesos, 0.016%, sem
  treino (MATH-500 7.9→48.1%, GSM8K 13.8→67.1%, IFEval 21.0→62.3%;
  re-run independente 51.0%). Pin novo em `flake.nix#qwen35-08b-q4km`
  (`NotCherry/…-GGUF` Q4_K_M, 541 903 776 B, LFS sha `0be511c8…`,
  `nix build` hash-verify PASS + sha256sum confirmado). Fallback
  `qwen35-08b-q4km-base` (unsloth sem edito, idem PASS).
  **Not-tested:** edito sobreviver à quantização Q4_K_M; ganhos com
  `--reasoning off`/ctx curto; engine boot em QEMU/metal. Ver
  `docs/HW_TEST.md` §Modelo de arranque. ISO `iso-hw-cuda` com S1+Q6_S2
  construída 2026-11-04 (8 323 596 288 B, sha `c2da2cfd…`;
  `docs/evidence/ISO_HW_CUDA_S1ASTRA_2026-11-04.md`).

- **Autoconhecimento + web (2026-11-03, in-tree)**: `nos-core::self_model`
  (catálogo de ~95 métodos com caps, domínios, efeitos; factos vivos;
  `limits` calculados; `entrails`; `pointers`) provado contra
  `nos-ipc::required_cap` por teste — o agente não pode "saber" uma cap que
  o daemon recusa. Prompt: bloco `## Self (live)` (Persona → Self → Memory →
  Nix → User), com motor/backend reais via `SelfEnv`. RPC `self.status`
  (NONE), `nix.audit` (NONE) e `net.research` (NET); CLI `self`, `nix-audit`,
  `research`; MCP `nsd.self`/`nsd.nix_audit`; HUD SIMBIOSE com CAPS e DÍVIDA.
  Lições Nix com pré-condições (`on_nixos`, `nixpkgs>=`, `has_nvidia`,
  `ring_disarmed`) — conhecimento que envelhece sem mentir. `memory.recall`
  passa a aceitar `path` (activacão por contexto nsgdb) e `entities`.
  `net.research`: DuckDuckGo Instant Answer (JSON, sem chave) + leitura
  https (8 s, 256 KiB), `http://`/`file://` recusados, offline =
  `net_unavailable` (nunca sucesso fingido).
  Evidência `docs/evidence/SYMBIOSIS_2026-11-03.md` §5.

- **Simbiose (2026-11-03)**: fábrica de apps real — `nos-factory::scaffold`
  gera Rust (`cargo`/`rustc`) ou Python, escreve, constrói e **executa**
  (`status: ok` só com stdout real; ausência de toolchain = `engine_absent`);
  RPC `factory.scaffold` (NONE, plano) e `factory.build` (AGENT_SPAWN|FS);
  cada app gerada leva `flake.nix`. Domínio Nix `nos-core::nix_domain`
  (factos vivos + 20 lições do `AGENTS.md`) no prompt (`## Nix (live)`), na
  rota S1/S2 e em `nix.status`/`nix.ask`. HUD ORB: painel `02c SIMBIOSE`
  (`orb.symbiosis`) com Nix/geração/ring/lições e plano de app.
  Evidência `docs/evidence/SYMBIOSIS_2026-11-03.md`.

- **nsgdb 1.5.0 → NixAIOS**: pin `ce5a5c70…` (`docs/NSGDB_VERSION.md` + flake input).
- `nos-memory::MemoryGovernor`: two-pass, null-scope, ADD-only follow-ups, SOUL markers.
- Backends: `neural` (default, FileStorage `memory/nsgdb/`) | `nsgdb_legacy` | `jsonl`.
- IPC: `memory.recall|health|remember|curate|decide`; kiosk read-only; nosmcp read tools.
- F5: CRDT path `mesh-crdt/` (migração one-shot desde `nsgdb/crdt/`).
- CI: `scripts/nsgdb-pin-check.sh` + `scripts/smoke-nsgdb-memory.sh`.
- Evidência: `docs/evidence/NSGDB_MEMORY_GOV_0.7.0-alpha.5.md`.
- Mesh frota: `--mesh-peer` / `NOSD_MESH_PEERS` / `services.nosd.meshPeers`
  (broadcast ∪ sementes, parse fail-closed); `PinError::Duplicate` distingue
  HB duplicado de replay; `scripts/smoke-mesh-unicast.sh`.
- Voz: WAV TTS único por chamada (`tempfile::TempPath`); `dispatch_blocking`
  tira handlers bloqueantes do worker async (parcial: keep-alive do ORB fica
  para a ADR `0118-voice-async-playback`).
- Memória: `validate_system_record` exige `scope` não-vazio (`ScopeRequired`).
- Tested: `cargo test -p nos-memory` (incl. neural scoped); hermes cognitive_bridge/OPAR; ipc/kiosk.
- **F7 telepathy usável entre peers mesh:** `memory.sync` / `nosctl memory-sync`
  via `KIND_MEMORY` no link Noise (`NOSD_JOB_BIND`); two-db `merge_remote`.
- Not-tested: QEMU/metal neural e2e; multilingual embed iso-hw; telepathy metal multi-host.
- **Closeout fórum 2026-11-02** (versão de pacote inalterada): harvest F9/F10
  (sumário `A6=play_ok`/`snd_denied`, `--since`, `trap`); N1 curate
  `performed:false` + RPC espelha `ok`; N2 `duplicate` separado de `applied`;
  telepathy M1 doutrina+cap; gate `M3b-audio` opt-in (nunca PASS sem HW);
  fixtures flake `mesh-crdt/` + empty SOUL no VM test. Commits `2d2fb01` /
  `da0100c`. Evidência:
  `docs/evidence/FORUM_CLOSEOUT_0.7.0-alpha.5.md`; memória:
  `docs/memory/SESSION_20261102_FORUM_CLOSEOUT.md`. Residual metal /
  `mesh-3vm-faults` / `iso-hw-cuda` AWAITING_HW.

## [0.7.0-alpha.4] — AIOS frota-3 (S1/S2 + HITL adapt + mesh Wi‑Fi/wired)

- ISO canónica `iso-hw-cuda`: S1 0.8B + S2 4B; Landlock após spawn CUDA.
- Mesh HW: UDP 19000, broadcast subnet (notes Wi‑Fi + AMD wired), node-id estável.
- `brain.adapt` HITL up/down/driver_hint; tier4 hot_load com VRAM sem ring.
- Cage: dbus session + espeak greeting; ORB 2K/cam/idle; logs `boots/<stamp>_<host>/`.
- `pack-hw-usb.sh`: `mv` same-FS (não duplicar ~20 GiB no VHDX).
- Opcional `iso-hw-cuda-legacy` (Pascal 580).
- Evidência: `docs/evidence/METAL_FLEET3_0.7.0-alpha.4.md`;
  memória: `docs/memory/SESSION_20261101_FLEET3_ALPHA4.md`.
- ISO sha256 `361cfa64…bc26f29f`; img `fa51efd5…5ea454a8`.
- Metal 240H HUD parcial (S2/CUDA/CAM OK; MIC/áudio/mesh/logs stick pendentes).

## [0.7.0-alpha.3] — metal iGPU/dGPU + iso-hw-cuda-4b

### Fixed

- **`nvidia-smi` SIGSEGV (glibc 2.44)**: `.dynamic` em `PT_LOAD` só `PF_R`
  após patchelf — OR `PF_W` via `scripts/fix-nvidia-elf-pf-w.py` + derivação
  em `images/hw-cuda.nix` / `hw-cuda-4b.nix`; check `nvidia-smi-elf-pf-w`.
- **Chromium → Slint no metal**: `HOME`/`XDG` graváveis sob `/var/lib/nosd`,
  `--crash-dumps-dir`, `NoNewPrivileges=false` em cinematic/auto (SUID
  sandbox), pin Mesa/Iris (`__EGL_VENDOR_LIBRARY_FILENAMES`), fallback Slint
  tenta GL iGPU antes de pixman (`modules/jarbas-cage.nix`).
- **`plan_route` no SOUL**: classificar o prompt do utilizador, não o
  `model_prompt` alongado (evita S2Deep em todo o chat curto).
- **Capacity híbrida**: `igpu_card`/`dgpu_card`, `probe_with_retry`, cache 20 s
  no smi (capacity + energia) para não spammar coredump.

### Added

- **`iso-hw-cuda-4b`**: um único GGUF `Qwen3.5-4B-Q6_K` (`active.gguf`), S1
  CUDA `NGL=99`, sem unit S2 :8082. Script `scripts/build-iso-hw-cuda-4b.sh`.
- Evidência metal pré-fix: `docs/evidence/hw/2026-10-31_1944Z/`.
- Evidência tranche: `docs/evidence/METAL_GPU_4B_0.7.0-alpha.3.md`.

### Changed

- `hw-cuda.nix`: S1 também em CUDA (`NGL=99`) + wait `/dev/nvidia0`.
- `slot_map`: prefixo `active*` força `ModelSlot::Active`.

### Artefactos

- ISO `67355245…` (7 648 026 624 B); img `5bacea85…` (9 795 796 992 B;
  FAT 2G). Downloads: `nixaios-hw-cuda-4b.iso` / `-logfat.img`.

Tested: checks ELF/DeviceAllow, testes Rust alvo, build ISO 4B.
Not-tested: metal re-boot com alpha.3.

## [0.7.0-alpha.2] — tier 4B Q6_K + contexto automático

### Added

- **Tier 4B Q6_K** (`Qwen3.5-4B-Q6_K.gguf`, 3 525 956 768 B, sha256
  `fdedd781…8f66`): o `brain.plan` escolhe-o antes do 4B Q4 quando há RAM
  (≥4,8 GiB); pensado para dGPU de **6 GB** (pesos ~3,3 GiB + KV/overhead).
  O 9B Q4 continua tier 4 para GPUs ≥6,6 GB livres.
- **Contexto automático**: `llama-server` arranca com `--fit on` (e
  `--fit-ctx`, mínimo default 2048) em vez de `-c` fixo; o engine escolhe o
  maior contexto/NGL que cabe na VRAM. `NOSD_LLAMA_CTX` explícito continua a
  mandar; `iso-hw` deixa de forçar 4096.
- `iso-hw` passa a incluir os **cinco** GGUFs pinados (0.8B Q4/Q8, 4B Q4/Q6,
  9B Q4).

### Changed

- Versão `0.7.0-alpha.2`; evidência e memória actualizadas.

### Fixed (pós-alpha.2, in-tree)

- **`DeviceAllow` NVIDIA** (metal CUDA): `char-nvidia-frontend` (só existe
  com módulo vGPU/Xen) e `char-nvidia-ctl` (não é alias; `/dev/nvidiactl`
  pertence ao major `nvidia`) removidos; falta era `char-nvidia rw`, que
  cobre `/dev/nvidiaN` + `/dev/nvidiactl`. Sob política `closed` implícita
  o systemd ignora aliases mortos em silêncio e a CUDA morria a abrir
  `/dev/nvidia0`. Fix nos 3 units (`nosd-engine@` e `nosd-engine` em
  `modules/nosd-engine.nix`; `nosd` e `nosd-engine-s2` em `images/hw-cuda.nix`)
  + check estático `checks.x86_64-linux.hw-cuda-deviceallow` no flake
  (avaliação dos DeviceAllow gerados; inclui o unit supervisionado do
  módulo). Ver lição 41 do `AGENTS.md`. Metal CUDA: AWAITING_HW.
- **Artefacto CUDA da revisão (voz+XDG)**: ISO `2ee396fd…` (17 221 165 056 B)
  no `.img` `4e339f37…` (21 516 779 520 B; tail-sha == ISO; 32 GB). Inclui
  `char-alsa`/`XDG_RUNTIME_DIR` (`92fb47a`), DeviceAllow e Ozone; substitui
  `4f5ce467`/`f7da2843`.
- **Artefacto CUDA republicado com o fix**: ISO `4f5ce467…`
  (17 220 493 312 B) no `.img` `f7da2843…` (21 515 730 944 B;
  tail-sha == ISO; pendrive ≥ 32 GB). Substitui o build `9470e357`
  (sem o fix). Ver `docs/evidence/hw/2026-10-31_0140Z/README.md`.

### Fixed (pós-alpha.2, in-tree — revisão de simbiose 2026-10-31)

- **Voz sob `DevicePolicy=closed`**: o `nosd` só tinha `char-drm` no
  `DeviceAllow`; `nos-voice` corre `arecord`/`aplay` como filhos do unit e
  herdava o cgroup de dispositivos — `/dev/snd` negado (microfone "avariado").
  Adicionado `char-alsa rw` e o check estático `nosd-voice-deviceallow`.
- **Motor supervisionado com as mesmas flags do spawn**: `services.nosdEngine`
  (template e concreto) passou a usar `--fit on --fit-ctx` (contexto conforme
  VRAM), `--reasoning off` e `--no-webui`, com opções `fitCtx`/`reasoning`;
  check `engine-fit-parity` impede a divergência voltar.
- **`nosd-early-score` com `path = [ coreutils ]`** (regra 5: nada de comandos
  nus em serviços NixOS).
- **Memória mais inteligente e menos ingénua**: recall dobra acentos,
  pontuação e hífen (`memoria`↔`memória`, `wifi`↔`Wi-Fi`) e ignora
  stopwords PT/EN; query só com stopwords devolve vazio em vez de todos os
  registos. O bloco `## Memory recall` passa a declarar que a memória é
  contexto não verificado (prompt-injection persistente), com teste.
- **Docs**: removida a frase "1777 do data-dir só pré-transmute" (a ISO aplica
  1777 a cada boot, intencional e já documentado em ARCHITECTURE/SECURITY).
- **Revisão completa** em `docs/SYMBIOSIS_REVIEW_2026-10-31.md`: mapa
  boot→JARBAS, simbiose NixOS, runtime de modelos, memória, fábrica wasm e
  roadmap P0–P2.

### Added (pós-alpha.2, in-tree — quota assinada)

- **Janela assinada pela chave do nó**: com identidade M6, o
  `skill_quota.json` leva Ed25519 sobre a forma canónica (peers ordenados) e o
  `node_id`; ao lado fica `skill_quota.anchor` com o último snapshot assinado.
  Seguintes casos passam a ser **recusados** (fail-closed) em vez de reporem
  orçamento: editar runs/fuel, remover a assinatura, assinar com outra chave,
  apagar a janela mantendo a âncora, repor um ficheiro assinado antigo
  (runs/fuel abaixo da âncora ou peer desaparecido de uma janela activa).
  Sem identidade (lab/release sem `NOSD_MESH_REQUIRE_SIG=1`) o arranque registra
  `skill quota signed=false` — a ausência de protecção é declarada.
- **Testes**: `signed_window_survives_a_restart_and_is_ed25519`,
  `editing_the_signed_window_is_refused`,
  `signed_window_without_signature_is_refused`,
  `window_signed_by_another_node_is_refused`,
  `deleting_the_window_with_the_anchor_present_is_refused`,
  `rollback_to_an_older_signed_window_is_refused` e o ponta-a-ponta
  `tampered_signed_quota_is_refused_by_the_daemon`.

### Added (pós-alpha.2, in-tree — quota persistida)

- **Janela da quota em disco**: `<data-dir>/skill_quota.json` (0600, tmp +
  fsync + rename + fsync do diretório) guarda runs/fuel por peer em epoch ms;
  um restart do `nosd` relê-a e **não devolve orçamento** a quem o esgotou.
  Janelas expiradas são descartadas no load/save; ficheiro corrompido ou de
  versão desconhecida = recusa global até o operador o remover (fail-closed);
  falha de gravação recusa a execução seguinte. O tempo é de relógio de parede
  (`Instant` não sobrevive ao processo): recuar só alonga a janela.
- **Testes**: `quota_window_survives_a_restart` (0600 + runs/fuel lidos do
  disco), `corrupt_quota_file_denies_instead_of_granting_a_fresh_budget`,
  `expired_window_is_pruned_on_load` e o ponta-a-ponta
  `skill_quota_survives_daemon_restart` (2 runs, kill, mesmo data-dir, 3.º
  recusado).

### Added (pós-alpha.2, in-tree — fuel no score + quota por peer)

- **Eficiência de fuel no score do Hermes**: `skill.run` acumula
  `skill/<nome>/fuel = {runs,total,max,limit}` no CRDT local; `skill.explain`
  dá até **10 pontos** (alvo = 10% do limite do perfil, `<3` execuções =
  5 neutro) e expõe `why = {fuel,fuel_avg,fuel_target,fuel_runs}`. Pesos
  rebalanceados para somar 100 (proveniência 25, caps 15, versão 30,
  tamanho 5, histórico 15, fuel 10).
- **Quota de execução por peer**: janela deslizante em memória, contabilizada
  pela identidade do transporte (`unix:<uid>`, `tcp:token:<geração>`,
  `jarbas-ui`); `NOSD_SKILL_QUOTA_{RUNS,FUEL,WINDOW_SECS}` (240 / 5 000 000 /
  60 s). A run é **reservada atomicamente sob o mutex antes de ler o módulo**
  (concorrência não estoura) e o fuel medido é cobrado depois; `Fuel` trap
  cobra o limite do perfil. Excesso = `ok:false` com `quota` no erro, sem
  executar. NixOS: `services.nosd.skillQuota{Runs,Fuel,WindowSecs}`.
- **Testes negativos**: quota de runs e de fuel ponta-a-ponta no daemon
  (`skill_peer_quota_rejects_after_budget`,
  `skill_peer_fuel_quota_rejects_after_budget`), reserva atómica
  (`quota_reserves_before_the_run_so_concurrent_calls_cannot_overshoot`),
  `NOSD_SKILL_QUOTA_RUNS=0` nega todos e
  `skill_fuel_telemetry_feeds_the_score` mede a queda do score.

### Changed (pós-alpha.2, in-tree — skills v2 / Wasm apps)

- **`nos-wasm` v2**: `SandboxPolicy` (frugal/app), `Inspection` de imports com
  allowlist (`aios_ai.infer`, `aios_log.write`), ABI v2
  (`result_ptr`/`result_len`, até 64 KiB no perfil app, bounds-checked), logs limitados e
  truncados, fuel medido (`ExecReport.fuel_used`) e cache de módulos BLAKE3
  por processo (`FrugalRuntime::shared`).
- **`nos-factory`**: promoção devolve `content_id` BLAKE3 (não FNV), ABI,
  bytes e fuel; draft vazio fica marcado `placeholder` (não há backend de
  codegen), em vez de ser apresentado como app sintetizada.
- **Skills**: `skill.install` e o hook do `skill.push` passam pelo mesmo
  `install()` (nome → tamanho/hash → publisher → caps → downgrade → contrato
  → gravação). `skill.run` concede `caps declaradas ∩
  NOSD_SKILL_GRANT_CAPS` (default `log`; `infer` exige opt-in), não executa
  skills sem wasm activo e devolve `fuel_used`, `fuel_limit`, `output`, `result` ou
  `result_b64`, `caps_declared`/`caps_granted`. `skill.list` junta o manifesto
  (versão, foco, caps, `kind`, ABI, perfil, proveniência). `nosctl skill-install
  --caps --profile app`; o perfil `app` (1 MiB/500k) só é aplicado com
  `NOSD_SKILL_APP_PROFILE=1` (o manifesto pede, o host decide). Em NixOS:
  `services.nosd.skillGrantCaps` (default `[ "log" ]`) e
  `services.nosd.skillAppProfile` (default `false`).
- **Testes**: 16 em `nos-wasm`, 24 em `nos-factory` e 10 de integração de
  skills (ABI v2 com log+result, import WASI recusado, INFER declarado sem
  grant, perfil app sem/com opt-in). Workspace verde no host Windows (421) e
  na derivação Nix/Linux `nix build .#nosd` (424, com integração).

## [0.7.0-alpha.1] — ORB UI + HW cinematic

### Added

- **HUD voz-primeiro** no `static/orb_lab.html`: microfone como controlo
  principal, áudio pedido antes do vídeo (falha de câmara não desliga o mic) e
  chat minimizado em **closed captions** + drawer de histórico. Em modo texto
  (voz desabilitada) a CC mostra o texto; em modo voz só STT/TTS.
- **Janelas HITL** a sério no HUD: tier server-authored (`ui.pending`), campos
  dinâmicos, `high` com checkbox + hold-to-authorize (~700 ms); o nonce nunca
  é exposto. `ui.action` continua a única mutação.
- **Mesh organograma**: `MeshEngine::topology()` + `health.mesh_nodes`
  (read-only) alimenta um grafo 3D pequeno (MST/WRK/MEM/CPU, pin/TOFU) e a
  lista de nós no drawer. No orb central, cada peer é uma mini-orb distante na
  cor do papel, ligada por **trilhos de dados bidireccionais** (duas pistas
  envio/recepção, travessas, segmentos animados e seta) — não pontos a disparar.
- **T/S honesto**: `~N t/s` estimado end-to-end (chars/4 ÷ tempo do turno),
  rotulado no `title`; `—` quando não medido. O engine ainda não expõe timings.
- **Energia CPU/GPU + termal**: `observe.energy.split` (CPU RAPL, GPU NVML)
  com joules/watts por componente e `thermal` (hwmon fan/pwm + recomendação
  `FAN +/FAN −/HOLD/SEM SENSOR`, advisory — o daemon não controla ventoinhas).
  A caixa 03 do HUD mostra CPU/GPU, gráfico duplo e termal sem crescer.
- **`iso-hw` 0.7-alpha1**: todos os GGUFs pinados (0.8B Q4/Q8, 4B Q4, 9B Q4)
  entram na imagem como symlinks content-addressed; default 0.8B Q4, ativação
  dos restantes por `brain.plan/accept` (HITL; 9B exige ring).

### Changed

- `http_lab`: `cognitive.trace` entra na allowlist do kiosk (NONE, metadados
  OPAR sem PII), com o teste de contrato existente.
- Painéis do HUD sem animação de `opacity` (o compositor do kiosk prendia
  layers em opacidade parcial); entradas por `transform`.
- **`iso-hw`**: `services.jarbasCage.uiMode = "cinematic"` por default — a
  primeira interface gráfica no metal é o orb web em Chromium `--kiosk`
  (fullscreen, sob cage), com fallback Slint. ISO de release/QEMU continuam
  `slint`.

## [0.6.0-alpha.16] — in-tree (V9: DAG + kiosk)

### Added (V9.1/V9.4/V9.5)

- **V9.1 `nos-core::dag`**: plano topológico com **isolamento de write-scope**
  por camada (prefixo por segmento: `src/a` conflita com `src/a/b`, não com
  `src/ab`); id vazio/duplicado, dep desconhecida e ciclo = erro; `write_scope`
  vazio = read-only declarado; máximo 256 nós; ordem determinística.
  RPC `dag.plan` (NONE, **não executa**) + `nosctl dag-plan <file.json>`.
- **V9.4 kiosk**: `observe.energy` e `ui.pending` na allowlist do bridge
  (`KIOSK_METHODS`), com teste de contrato que exige caps NONE/INFER/HITL e
  recusa 17 métodos de escrita.
- **V9.5**: convenção `Tested:`/`Not-tested:` no corpo dos commits (lição 36).

### Changed

- `http_lab`: allowlist extraída para `KIOSK_METHODS` (uma fonte, testável).

## [0.6.0-alpha.15] — in-tree (V8: doações OMH + eco-tokens)

### Added (V8.1–V8.6 — doações oh-my-hermes + eco-tokens)

- **V8.1 `nosmcp`**: bin MCP **stdio read-only** (`initialize`, `tools/list`,
  `tools/call`, `ping`, `-32700` com `id:null`). Tools: `nsd.health|score|
  energy|pending|trace`. Zero escrita, zero allowlist nova no kiosk. O
  transporte vive em `nos-mcp::stdio` (testável com `Cursor`).
- **V8.2 `nos-core::reported_rate`**: toda a percentagem carrega
  `numerator_of`/`denominator_of`/`excluded`; denominador 0 ⇒ `percent:null` +
  `basis=no_observations`; `meets_target` falha o não medido. Aplicada à
  **cobertura de energia** (`observe.energy.coverage`).
- **V8.3 `nos-core::trim_diagnostic`**: ANSI (CSI/OSC/escapes), 5 head + 15 tail
  em falha, cauda de 20 no sucesso, cap 1000 chars em fronteiras de `char`.
  Usado nos erros de chat e no `doctor`.
- **V8.4 `doctor`**: coluna `reported|verified|unmeasured` por subsistema;
  `unmeasured` nunca é `ok`.
- **V8.5 ordem de prompt**: marcadores `## Persona (SOUL)` → `## Memory recall`
  → `## User` (prefixo fixo estável para KV cache) + 2 testes de contrato.
- **V8.6 `nos-core::LoopGuard`**: breaker por fingerprint no
  `self.heal.evaluate` (recusa `loop_detected`, `reverted:false`); substitui o
  `git checkout .` do eco-tokens por recusa honesta.

### Fixed (D1 — inventário de spawns)

- **`nvidia-smi` por allowlist absoluta** (`nos-core::proc`): candidatos
  fechados (`/run/current-system/sw/bin/nvidia-smi`, `/usr/bin`, …) e override
  `NOSD_NVIDIA_SMI` **só se absoluto**; um `PATH` manipulado deixa de decidir o
  binário. Usado em `nos-observe::energy` **e** `nos-cortex::capacity` (antes
  ambos falhavam em NixOS → `nvml_absent` mesmo com GPU).
- **`path` do unit**: `systemd` (journalctl) e `coreutils` (`timeout`) — sem
  eles o snapshot de journal e `voice.stt` ficavam vazios/falhavam em NixOS.
- **`docs/SPAWN_INVENTORY.md`**: inventário dos 35 `Command::new` com resolução
  e dívida (nomes nus vs caminho absoluto).
- Opção `services.nosd.nvidiaSmi` (null = allowlist).

## [0.6.0-alpha.14] — 2026-10-29 (Plano vampiro V0–V7)

### Added (Plano vampiro V0–V7 — absorção selectiva do OpenJarvis)

Nada do runtime Python/Node foi incluído: só ideias/specs reimplementadas em
Rust, atrás do `CapGate`. Evidência:
[`docs/evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md`](docs/evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md).

- **V0** `nosctl doctor` (agrega health/score/observe/modelos/transmute/voz/
  firstboot; `--json`; exit 1 em falha, SKIP≠OK) e `nosctl config-show`
  (`config.show`: configuração efectiva com env `NOSD_*` **filtrado por nome**
  — TOKEN/SECRET/KEY/PASS/PUBLISHER nunca saem). Pesos do score declarados:
  `services.nosd.scoreWeights` → `NOSD_SCORE_WEIGHTS` (`nos-core::metrics`,
  normalizado, inválido = default + motivo no `config.show`).
- **V1** energia por janela: crate `nos-observe::energy` (RAPL
  `/sys/class/powercap/…`, NVML via `nvidia-smi`, `tdp_estimate` só com
  `NOSD_TDP_W`, senão `absent` = zero joules). `observe.energy` devolve
  J/W/`source`/acumulado; `chat`/`chat.submit` anexam bloco `turn`.
  Wrap do contador = `rapl_wrap` (nunca joules inventados).
- **V2** custo: `nos-core::cost` (local €/kWh + contrafactual de nuvem
  sempre com `ref_date`/`ref_model`/`HYPOTHESIS`) e
  [`docs/PRICE_REFERENCE.md`](docs/PRICE_REFERENCE.md); tokens `chars/4`
  rotulados `char_heuristic`.
- **V3** `cognitive.trace`: ring buffer OPAR (64) no Hermes exposto **só** como
  domínio + comprimentos das fases + `remembered`/`seek` (redacção por
  construção; teste com sentinela garante que notas/PII não saem).
- **V4** `ui.pending`: fila HITL com `tier` (`trivial/low/medium/high`,
  `nos-core::ui_window`) e TTL agregado dos grants — **sem** `confirm_id` e
  **sem** memória de permissões (regra 2).
- **V5** crate `nos-mcp`: JSON-RPC 2.0 `initialize`/`tools/list`/`tools/call`,
  registry server-authored, deny-by-default por `CapGate` (tool sem caps =
  deny), side-effect exige HITL; `mcp.list`/`mcp.call` no nosd. `stdio`/SSE
  **NOT_DONE**.
- **V6** `skill.import`: parser `SKILL.md` (agentskills.io) → manifesto S1
  (`skill/<n>/meta`, `kind=declarative`, BLAKE3) com caps em **allowlist**
  (`log`, `memory:read`, `aios_ai.infer`, `net:read`); `code:execute`/`shell`/
  `file:write`/`network:fetch` recusados por nome. Não activa nada.
- **V7** `nosctl dash [--plain] [--interval N] [--count N]`: HUD de terminal
  zero-deps (health/score/energia/fila HITL) para serial/metal.
- **HUD Slint**: `energy_line` + `hitl_pending` no topo e `tick_line` com o
  último domínio OPAR (poll no ciclo existente, sem threads novas).
- **Honestidade da energia**: cada amostra traz `scope`
  (`cpu_package`|`gpu`|`machine_estimate`|`none`) — RAPL mede a CPU, NVML a
  GPU; o custo local é **limite inferior** excepto com `tdp_estimate`.
- **`scripts/smoke-vampire.sh`**: smoke V0–V7 contra um `nosd` a correr
  (config-show/doctor/energy/chat/trace/pending/mcp/skill-import/dash), com
  asserções de não-vazamento (`confirm_id`, `NOSD_IPC_TOKEN`).
- `mcp.call` e `skill.import` entram em `SIDE_EFFECT_METHODS` (OPAR).
- **Fix**: `nosctl` corria para stack overflow (`--help`) com a superfície
  actual — `clap` constrói a árvore na stack (1 MiB no Windows); `main()`
  passou a correr o runtime numa thread de 16 MiB.

### Added (Brain self-upgrade — HITL completo)

- **`brain.plan` / `brain.accept`** (nosd RPC + `nosctl brain-plan`/`brain-accept`):
  a AI observa o hw real (posture RAM/VRAM/DRM) e propõe o tier alcançável
  mais alto — **tier 2** Qwen3.5-0.8B **Q8_0** (mesmo modelo, precisão
  4→8-bit), **tier 3** Qwen3.5-4B Q4_K_M (5x parâmetros, CPU/iGPU), **tier 4**
  Qwen3.5-9B Q4_K_M (alvo dGPU supervisionada, exige ring armado). A card
  HITL mostra **custo exato** (bytes/MB pinados; RAM/VRAM rotulados "est."),
  ganho honesto (sem tokens/s prometidos), rollback local (o anterior fica
  em disco) e o **sha256 pinado** do LFS do HF — o accept faz fetch →
  **verificação do sha256** (diverge = apagado, fail-closed) → hot_load →
  OPAR. Executa NUNCA automático (Escalate, grant `brain.upgrade`
  server-authored, single-use). Gancho pós-boot (`NOSD_BRAIN_PROPOSE=1`)
  abre a card no HUD a cada 30 ticks com memória anti-spam 24h no CRDT
  (`brain/upgrade/last`).
- Registo de tiers pinado em `nos-hermes/src/brain_upgrade.rs` (SRI +
  bytes exactos auditados no tree `unsloth/...-GGUF`).

### Added (Brain embutido iso-hw — ADR-0118 addenda)

- **`packages.qwen35-08b-q4km` / `qwen35-08b-q8`** (flake): GGUF
  Qwen3.5-0.8B (unsloth, Apache-2.0) como **derivação Nix content-addressed**
  (pin SRI + tamanho, focos do tree HF LFS) — p/ embutir na `iso-hw`
  (**NUNCA release**, S1.2 mantida para release): brain começa a correr no
  primeiro boot **sem download**. Pós-boot, modelos maiores/melhor
  quantização = proposta JARBAS via card HITL (`blob.fetch` server-authored)
  conforme `docs/adr-addenda/0118-qwen-embedded-brain.md` (tiers de
  capacidade → candidato, nunca auto-switch).

### Fixed (T0 — dívida de integração fechada no código)

- **SKILL_PUSH desbloqueado:** o envelope de skill (módulo em base64 ~4/3)
  não cabia no `MAX_PROMPT` de 8 KiB — o teto de 512 KiB era inalcançável
  (módulos >~6 KiB morriam em "prompt acima do limite" antes de o limite de
  módulo ser consultado). `nos-mesh::job::max_prompt_for(kind)` dá teto
  próprio a `KIND_SKILL` (768 KiB); teste e2e com módulo de 300 KiB
  (`skill_push_aceita_modulo_grande_e_recusa_acima_do_cap`) e recusa acima
  de `MAX_SKILL_BYTES`.
- **Score de skills clamped a 0-100** (a soma teórica chegava a 120 com
  proveniência signed + sem caps + versão máxima + histórico perfeito).
- **`hitl_confirmed` do cliente deixa de autorizar `audio.volume`** (regra 2
  do AGENTS.md): a autorização vem da identidade — cap HITL do perfil decide
  plan-only vs apply. `pipewire_volume_plan` perde o parâmetro booleano.
- **`published_at_ms` deixou de ser campo morto** no manifesto de skills
  (instante de publicação observado pelo nó; honestidade: não é autoridade
  de ordem — a ordem continua a vir do VV do CRDT). Comentário do header do
  `skills.rs` actualizado (hash é BLAKE3 desde M8.1, não FNV).

### Changed (runtime/runtime, T0.4-T0.7)

- **`NOSD_ENGINE_DIR` removido** do `services.nosd` (env morto — nenhum
  ficheiro Rust o lia; o rpath dos engines no flake não depende dele).
- **`services.nosd.package` propaga-se** para `nosd-early` e hotswap/firstboot:
  os scripts deixam de hardcodar `pkgs.nosd` (builds customizados deixam de
  falar com outro binário).
- **StartLimit no unit `nosd`** (`startLimitIntervalSec=120`,
  `startLimitBurst=5`) — anti-crash-loop era atributo do UNIT, não de
  serviceConfig (lição 2; o cage já tinha, o daemon não).
- **1777 de `/var/lib/nosd` só pré-transmute**: o ExecStartPre do cage passa
  a devolver o sandbox 0750 nosd em aios-mode — os marcadores que disparam
  serviços root (`hotswap-aios.request`) deixam de ser forjáveis por
  qualquer user local em host pós-transmute. Na ISO live (pré-transmute) o
  1777 mantém-se (EACCES pós-Q5).

### Added (T3.1/G3 — ring Nix por campos tipados)

- **`nix.propose fields={hostname,locale,timezone,allow_unfree}`:** o staging
  nasce de um template fixo (`nix_managed::render_ai_managed`) com valores
  charset-restrito (sem `"`/`\`/metacaracteres Nix) — injecção no módulo
  recusada por construção; a deny-list e o syntax check continuam como 2ª e
  3ª camadas do caminho livre.
- **`services.nosdNixRing.twoPhase` (default true, G3.2):** apply root passa a
  duas fases — fase 1 = copy staging + build-vm + `nixos-rebuild test`
  (**boot default intacta**); o `switch` só corre na fase 2, que exige
  `nix-confirm.request` (escrito por `nix.confirm` HITL, TTL 600 s).
  Anti-brick por omissão; rollback/header mantidos.

### Added (G2/T2 — motor como serviço supervisionado)

- **`services.nosdEngine.supervise = true`:** cria `nosd-engine.service`
  (llama-server supervisionado pelo systemd, sandbox completo declarado no
  unit: `ProtectSystem=strict`, `DevicePolicy=closed`, StartLimit no UNIT,
  OOM isolado do cage) e liga o nosd por HTTP externo
  (`NOSD_LLAMA_SERVER=http://127.0.0.1:<port>`, backend attach já existente).
  Default OFF — o caminho managed/spawn (unload → cpu-echo → reload dos VM
  tests) continua intacto.

### Security (G1 — armadura de persistência e identidade)

- **Âncora anti-rollback assinada pela identidade do nó** (T1.1): a ancora do
  log CRDT passa a `<bytes> <linhas> <sig b64>` sobre
  `NSGANCHOR bytes lines`. Com identidade, ancora sem assinatura (forjada)
  é recusada — quem escreve no data-dir deixa de conseguir fabricar um
  rollback com ancora nova (só lhe resta apagá-la: fail-closed, igual hoje).
  Sem identidade (lab), o formato de 2 campos mantém-se.
- **Pins TOFU persistidos** (T1.2): `<keys-dir>/pins.dat` (`peer <id> <pk hex>
  <last_seq>`), partilhado entre HB UDP e listener de jobs. Um restart deixa
  de re-fixar às cegas (primeiro contacto pós-restart fixava de novo um MITM
  a ouvir na janela TOFU) e o anti-replay sobrevive ao restart. Linha cujo id
  não deriva da pk é ignorada ao carregar (fail-closed).
- **Seq do HB persistido** (T1.3): `<keys-dir>/hb.seq` — arranque de
  `max(epoch_ms, persistido+1)`; a janela de "Replay para sempre" se o
  relógio recuar entre restarts fecha (checkpoint a cada 16 HBs).
- **`static mut APPLIED` do Landlock → `AtomicU32`** (T1.5): fim do
  "atomic-ish read" UB-potencial; `LandlockStatus` passa a `#[repr(u32)]`.
  (`NOSD_LANDLOCK=1` já activo no unit de produção desde 0.5.x.)

### Fixed (G5.1 — gate)

- **`gate-boot-to-jarbas.sh --strict`:** SKIP em estágio obrigatório
  (`NIXAIOS_GATE_REQUIRED`, default substrato + M3 + MQEMU) passa a FAIL
  com `detail=strict_skip` — a cultura "SKIP reprova no aceite" está
  aplicada de facto. M4/M5 continuam fora por serem AWAITING_HW honestos.

### Added (UI híbrida JARBAS)

- **`services.jarbasCage.uiMode = "auto"`**: escolhe Chromium só com
  `MemAvailable >= chromiumMinAvailableMiB` (2048) **e**
  `chromiumGpuValidated = true`; caso contrário Slint. Os modos explícitos
  `slint`/`cinematic` ficam intactos e o falhanço do Chromium (lab em baixo,
  renderer a sair) cai para Slint pelo mesmo caminho software/pixman.
- **HUD Chromium ligada ao Hermes**: painel de estado do sistema + janelas
  declarativas/HITL (`ui.list`/`ui.action`) no lab, a par do orb em tempo
  real; o caminho Slint lê agora `audio_on`/`video_on`, nível e movimento de
  `orb.get` a cada 200 ms sem aumentar as consultas de saúde/score/janelas.

### Security

- **Bridge HTTP sem `ipc.token`:** `GET /` deixa de injetar o token do
  processo no HTML; o kiosk usa uma chave bootstrap 0600 trocada por cookie
  `HttpOnly`/`SameSite=Strict`, com perfil `AuthContext::JarbasUi`, origem
  exacta de loopback, marcador `X-Jarbas-UI-Request`, allow-list de métodos e
  recusa de `token`/`caps` no payload do browser. Sessão com TTL deslizante
  (renovada pelo poll do HUD) e chave fail-closed contra plantação/symlink no
  `data-dir` 1777. `GET /transmute` passa a 403 no kiosk.

### Added (M5.1)

- **Check `mesh-3vm-faults`**: 3 VMs com latencia (`tc netem`), perda total,
  `kill -9` + restart com replay do log `NSGC` e particao de link.

### Fixed (M8.1)

- **Delta do CRDT:** o VV passa a ter contador global por peer; a 2a chave
  escrita pelo mesmo no deixa de ficar fora do `crdt.sync`.

### Fixed (M6)

- **HB depois de restart:** o `seq` passa a ter base em epoch ms; o no
  reiniciado deixa de ser recusado como replay pelos peers.

## [0.6.0-alpha.11] — 2026-10-27 (Frota 3-VM com falhas)

### Added

- **`mesh-3vm-faults`:** 3 VMs com latencia (netem), perda total, `kill -9`
  (replay `NSGC`) e particao de link; CI e espelho local passam a 6 checks.

### Fixed

- **CRDT delta:** VV com contador global por peer; a 2a chave do mesmo no
  deixa de ficar fora do `crdt.sync`.
- **HB pos-restart:** `seq` com base em epoch ms; o no reiniciado deixa de ser
  recusado como replay.

## [0.6.0-alpha.10] — 2026-10-27 (S2c + M8.1d em nsgdb)

### Security

- **S2c autoria de skills:** historico no score, autor persistido no manifesto,
  push assinado verificado no destino (e2e em VM), revogacao de publisher
  (`skill/revoked/<hex>`, recusa mesmo com assinatura valida) e downgrade so por
  grant HITL `skill.downgrade`.
- **M8.1d anti-rollback:** ancora (`seg.anchor`/`crdt.anchor`); log em disco mais
  pequeno do que a ancora impede o arranque (fail-closed).

### Changed

- **M8.1d backend nsgdb:** log do CRDT em `nsgdb/crdt/seg.bin` (`NSGC`,
  length-prefixed) por omissao; `NOSD_CRDT_BACKEND=jsonl` mantem o legado e um
  `crdt.jsonl` existente e migrado uma vez para `crdt.jsonl.migrated`.

### Added

- **CLI de skills:** `skill-sign`, `skill-revoke` e
  `--focus/--version/--publisher/--sig/--confirm-id` no `skill-install`.
- **CI local:** `scripts/ci-local.sh` replica o workflow, com frota sequencial.

### Tests

- `mesh-3vm-faults`: 3 VMs com netem/perda/kill/particao, PASS; `mesh-2vm-crdt`
  com `NSGC`; `mesh-2vm-skills` com a cadeia assinada completa.

### Known issues

- firstboot offline acrescenta `--no-download` duas vezes: o passo `transmute`
  falha com `exit=2` e a imagem cai para a GUI. Correcao em alpha.12.

## [0.6.0-alpha.8] — 2026-10-27 (S-04 fechado: HITL só por grant)

### Security

- **`hitl_confirmed` é ignorado no wire.** A única prova é `confirm_id`
  server-authored (janela `hitl_confirm` + `ui.action`, single-use,
  kind-bound, TTL 5 min). Nem root/token o converte em consentimento.
- O gate `SITE_POLICY` (escalate/confirm) passou a exigir grant por
  domínio (`nix.apply`, `wifi.connect`, `factory.install`, ...).

### Changed

- `nosctl`: `--confirm` removido de `fetch`/`nix-apply`/`wifi-connect`/
  `transmute`; usar `--confirm-id`. Novo `nosctl consent <kind>`.
- Firstboot obtém o grant com `nosctl consent transmute`.

## [0.6.0-alpha.9] — 2026-10-27 (Frota: identidade→skills)
### Docs (memoria)

- Licoes da sessao (systemd, Wayland, tokio/EPIPE, Nix, seguranca e
  ambiente) em `docs/memory/SESSION_FLEET_M6_M9.md`.


### Added (hw)

- **`iso-hw`**: imagem de teste em hardware com logs persistentes numa FAT32
  `NIXAIOSLOG` (journal, firstboot/hotswap/cage, dmesg, lspci/lsusb). Ver
  `docs/HW_TEST.md`.

### Added (hw)

- **Modelo de arranque Qwen3.5-0.8B na `iso-hw`**: llama.cpp inclu?do,
  GGUF Q4_K_M do unsloth (508 MB, Apache-2.0) baixado no first boot via
  transmute/HITL; overrides `NOSD_BOOT_MODEL_{REPO,FILE,LOCAL}` e
  `NOSD_BOOT_MODEL_ONLY`. Ver `docs/HW_TEST.md`.

### Changed (M8.1d)

- **Backend nsgdb no CRDT:** log em `nsgdb/crdt/seg.bin` (`NSGC`,
  length-prefixed) por omissao. `NOSD_CRDT_BACKEND=jsonl` mantem o legado, e um
  `crdt.jsonl` existente e migrado uma vez para `crdt.jsonl.migrated`.

### Security (M8.1d)

- **Anti-rollback do log CRDT:** ancora `crdt.anchor` (bytes + linhas), gravada
  ao abrir, ao compactar (antes do rename) e a cada 32 linhas. Um log em disco
  mais pequeno do que a ancora impede o arranque (fail-closed).

### Security (S2c)

- **Downgrade via HITL:** versao inferior exige grant `skill.downgrade`
  (`--confirm-id`); sem grant continua recusado.

### Tests (S2c)

- **E2E do push assinado** em `mesh-2vm-skills`: assinar com a chave do no,
  instalar com autoria, empurrar, verificar `provenance=signed` no destino e
  recusar apos revogacao.

### Added (S2c)

- **CLI de autoria:** `nosctl skill-sign`, flags `--focus/--version/--publisher/--sig`
  no `skill-install` e `nosctl skill-revoke <hex>`.

### Security (S2c)

- **Revogacao de publisher:** `skill/revoked/<hex>` no CRDT; a assinatura deixa
  de valer mesmo sendo valida (install e push).

### Security (S2c)

- **Push assinado:** o emissor reencaminha `publisher`/`sig` e o destino
  verifica antes de gravar (inv?lida = recusado; ausente = `peer`).

### Changed (S2c)

- **Assinatura persistida:** o manifesto guarda `publisher`/`sig` quando o
  install e assinado (pre-requisito do push assinado).

### Added (S2c)

- **Historico no score:** `skill.run` conta sucesso/falha em
  `skill/<n>/stats`; `skill.explain` usa `20*ok/(ok+fail)` (>=5 execucoes) e
  expoe `history`/`runs` no `why`.

### Security (S2b)

- **Manifesto validado:** `caps` declaradas restritas a `aios_ai.infer`/`log` e
  **downgrade recusado** por `focus` (vers?o monot?nica, fail-closed).

### Security (S2a)

- **Autoria de skills:** `skill.install` aceita `publisher` + `sig` sobre
  `name|focus|version|hash`; assinatura inv?lida ou publisher fora da
  allowlist (`NOSD_SKILL_PUBLISHERS`) ? recusada antes de escrever.

### Security (S1)

- **Skills com BLAKE3:** o id de conte?do passa a commitment de 64 hex e cada
  skill tem manifesto (focus/version/caps/provenance).
- **Escolha audit?vel:** `skill.explain <focus>` pontua candidatas, escolhe a
  melhor e grava `focus/<f>/active` + `why` no CRDT.

### Security (M8.1e)

- **Log CRDT autenticado em repouso:** cada linha ? assinada com a identidade
  Ed25519 do n?; replay recusa linhas sem assinatura ou com assinatura inv?lida
  (fail-closed quando h? identidade). Editar o ficheiro deixa de alterar estado.

### Fixed (M8.1c)

- **Log CRDT:** linhas truncadas (crash) s?o ignoradas e o log ? compactado no
  arranque acima de 64 linhas (snapshot + `rename` at?mico).

### Added (M8.1b)

- **Estado duravel:** log append-only `<data-dir>/crdt.jsonl` com replay no
  arranque; abrir o log e fail-closed. `mesh-2vm-crdt` continua PASS.

### Docs (skills)

- Requisitos de segurança e qualidade das publicações wasm e algoritmo de
  escolha entre candidatas do mesmo foco: `docs/SKILL_TRUST_AND_SELECTION.md`.

### Added (M9.1)

- **SKILL_PUSH:** `skill.push <addr> <nome>` transporta o `.wasm` pelo link
  cifrado e o destino **s? activa se o hash bater**; `skill-install` aceita
  `.wat`/`.wasm`. `mesh-2vm-skills` PASS (A?B, corre `ident(9)` em B).

### Changed (M8.1)

- **CRDT por delta:** `crdt.sync` passou a pull por `version_vector` + push do
  `delta_since(vv)`, em vez do estado completo. `mesh-2vm-crdt` continua PASS.

### Security (M6.1)

- **Link de jobs cifrado:** Noise XX (X25519 descart?veis) + `BIND` Ed25519
  ligado ao `handshake_hash` e verificado contra a chave fixada no mesh.
  Sem modo plaintext. `mesh-2vm-jobs` e `mesh-2vm-crdt` passam pelo canal.

### Added (M9)

- **Skills wasmi:** `skill.install` (wat/wasm_b64) content-addressed,
  `skill.list`/`skill.run` e `nosctl skill-*`; registo `skill/<n>/active` no
  CRDT; sandbox frugal (fuel 100k, heap 256 KiB, zero capacidades).
  Testes: identidade, fuel exhausted, infer negado.

### Added (M8)

- **Estado replicado:** CRDT (LWW + version vector) sobre o link autenticado do
  M7 (`kind=2`), RPC `crdt.put/get/sync` e `nosctl crdt-put/crdt-get/crdt-sync`;
  `mesh-2vm-crdt` PASS (converg?ncia nos dois sentidos, 2 entradas).


### Added

- **M6:** identidade Ed25519 (`nos-mesh::ident`), HB assinado com `seq`,
  pinagem TOFU, anti-replay e allowlist; `mesh-2vm-signed` PASS.
- **M7:** jobs remotos (`nos-mesh::job`) com frames assinados sobre TCP,
  `NOSD_JOB_BIND`, RPC `job.submit` e `nosctl job-submit`;
  `mesh-2vm-jobs` PASS (A sem modelo, B responde verificado).

## [Unreleased] — M5 lab (mesh 2-VM)

### Added

- **`checks.x86_64-linux.mesh-2vm`:** dois NixOS na mesma VLAN (UDP 19000)
  validam `mesh_peers == 1`, HB com `PK` (TOFU) e eleição stub
  (Master/Worker). Sem alteração do binário (ISO alpha.7).
- **`nos-mesh::tofu_settles_after_window`:** regressão do settle de TOFU
  (7.2 s) que destranca ROLE/SkillSync.

### Evidence

- [`docs/evidence/MESH_2VM_0.6.0-alpha.7.md`](docs/evidence/MESH_2VM_0.6.0-alpha.7.md)

## [0.6.0-alpha.7] — 2026-10-27 (ring Nix desarmado + kill switch root)

### Security

- **R-03 (decisão):** o ring Nix fica **desarmado** (`armRing = false`);
  ADR addendum `0118-nix-ring-decision.md` com os critérios para religar.
- **R-04:** o helper root deixou de confiar no pedido — exige
  `NOSD_NIX_RING=1`, `target` fixo, `staging` confinado ao data dir,
  `flock`, `timeout` e PATH explícito. Rollback com a mesma gate.
- VM test prova que um pedido forjado em `/var/lib/nosd` (1777) é ignorado.

## [0.6.0-alpha.6] — 2026-10-27 (S-08/S-09 hardening)

### Security

- **S-09 socket `0660`:** `serve_unix` faz `chmod 0660` após o bind e aborta se
  falhar. Só `nosd` e o grupo `nosd` ligam ao IPC (auth continua `SO_PEERCRED`
  + caps).
- **S-08 regressões:** socket 0660, métier desconhecido fail-closed, admin sem
  token negado, frame com tamanho `u32::MAX` rejeitado, `DENY_UNKNOWN` para
  método não listado.

## [0.6.0-alpha.5] — 2026-10-27 (Boot→firstboot→JARBAS headless)

### Fixed

- **firstboot em `ttyS0`:** os gatilhos só aceitavam `/dev/tty[0-9]*`; a ISO
  arranca com `console=ttyS0` e o auto-login cai numa sessão serial, pelo que
  o questionário **nunca corria**. Adicionado `ttyS*`/`hvc*` nos três sítios.
- **`nosd-hotswap-aios.service` exit 127:** shebang `#!/usr/bin/env bash` sem
  `bash`/`tee`/`systemctl` no `path` do unit. `path` explícito (e no
  `nosd-boot-persist-check`).
- **hotswap reiniciava o próprio `nosd`** que servia o `transmute` por RPC →
  `nosctl exit=1`. Só reinicia se o health estiver offline.
- **`transmute` contava o `systemctl` sem root/polkit como erro.** O hotswap é
  disparado pelo `.path` (PathExists); `systemctl` passou a best-effort.
- **`nix build` após `cargo test` no host:** `packages/nosd/target/` (GBs) era
  copiado read-only para a store (`cp: Read-only file system`). `src` com
  filtro (`builtins.path`).
- **Primeiro frame preto** no arranque automático (VT activa): `chvt` para a
  VT do kiosk no `ExecStartPre` root, antes do `cage`.

### Added

- `services.nixaiosTransmute.firstbootAuto` (ISO/lab): defaults + rede 8 s +
  `transmute --no-download`; overrides `NOSD_FIRSTBOOT_ASK=1` /
  `NOSD_FIRSTBOOT_DOWNLOAD=1`.
- `ask_def` no firstboot (default por pergunta em modo auto).

### Evidence

- [`docs/evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md`](docs/evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md)

## [0.6.0-alpha.4] — 2026-10-27 (Boot→JARBAS Slint em cage/DRM)

### Fixed

- **`/run/user` fantasma:** logind monta um tmpfs em `/run/user` no 1º login e
  escondia `/run/user/nosd`; o wrap morria com `mkdir: EACCES`. Runtime dir
  passou a `/run/nosd/user` (tmpfiles + `ExecStartPre` root).
- **`PATH` sem `systemctl`** fazia o `waitScript` apagar `/run/seatd.sock`
  vivo (seatd-launch depois falhava). `pkgs.systemd` no `path` + caminho
  absoluto.
- **`StartLimitBurst`/`StartLimitIntervalSec` em `serviceConfig`** (ignorados
  pelo systemd) → agora no unit; anti-loop real (`start-limit-hit`).
- **`cargoExtraArgs` removido no nixpkgs 26.11** era ignorado: ISO saía sem
  `slint-ui`. `cargoBuildFlags` → HUD Slint no binário.
- **Panic tokio aninhado** (`rpc.rs`): `#[tokio::main]` + `rpc_blocking`.
- **winit→X11/Xwayland** e **`LIBGL_ALWAYS_SOFTWARE` a quebrar EGL**: forçar
  `WINIT_UNIX_BACKEND=wayland` (só no filho) e `WLR_RENDERER=pixman` no retry.

### Added

- HUD: header fixo no topo, orb 0.55, linhas elididas, `XDG_CACHE_HOME`
  gravável (fontconfig).
- Evidência com screenshot do scanout QEMU.

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
