# AGENTS.md — regras persistentes do NixAIOS

Este ficheiro é memória operacional para agentes/contribuidores. Leia antes de
mexer no código. O plano vivo está em [`docs/IMPROVEMENT_PLAN.md`](docs/IMPROVEMENT_PLAN.md);
a tranche actual + próximos passos (V8) em
[`docs/VAMPIRE_PLAN_OPENJARVIS.md`](docs/VAMPIRE_PLAN_OPENJARVIS.md) §12.

## Norte

NixAIOS é a bancada NixOS/Linux do `neural-os-core` (ADR-0118), **não** um fork
do Ring0. O aceite primário é **M3 — Boot→JARBAS Jarvis-like**, seguido de
**M4 metal** e **M5 frota**. Ver [`docs/BOOT_TO_JARBAS.md`](docs/BOOT_TO_JARBAS.md)
e [`docs/REMIGRATION_PLAN_LEDGER.md`](docs/REMIGRATION_PLAN_LEDGER.md).

Versão corrente: **0.7.0-alpha.2** (in-tree; última ISO publicada `0.6.0-alpha.11`). Volid ISO: **`NIXAIOS051`**.

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
.eco/                    MCP eco-tokens (4 tools) + RULES.md — ver docs/ECO_TOKENS.md
opencode.json            config OpenCode (mcp.servers.eco-tokens + instructions)
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
13. O VV do CRDT precisa de contador **global por peer**; por chave esconde a
    2a chave nova do delta (`crdt.sync` responde ok com `sent=0`).
14. O `seq` do HB tem de ter base em **epoch ms**: a comecar em 1, os peers
    recusam o no reiniciado como Replay para sempre.
15. QEMU `-netdev socket,listen` aceita **uma** ligacao; para N VMs usar um
    relay (1 datagrama = 1 frame) via `socket,udp` + relay userspace. `mcast`
    nao atravessa o WSL e sem root nao ha bridge/TAP.
16. No guest a NIC e `ens3`/`enp0s3` (nomes previsiveis), nunca assumir `eth0`.
    `255.255.255.255` precisa de rota (senao `ENETUNREACH`); com IP estatico
    usar o broadcast da sub-rede (`192.168.50.255`).
17. A ISO live tem o **firewall NixOS ligado**: UDP 19000/TCP 19001 morrem no
    INPUT. Os checks desligam-no; uma demo tem de o desligar/abrir.
18. Marcador de shell por serial tem de ser **avaliado** (`echo __RC=$?`),
    nunca literal: a consola ecoa a linha e o literal faz match antes da saida.
19. Fechar a janela GTK mata o QEMU com `rc=0` e aborta o teste; em automacao
    usar **VNC** + `screendump` (o GTK e so para ver).
20. `screendump` do virtio-gpu pode sair **preto** com `jarbas-cage` active
    (VT/plane): VNC e a prova visual; nao guardar capturas pretas como HUD.
21. `images/iso.nix` faz `mkForce false` no sshd; uma demo precisa de
    `lib.mkOverride 10 true` (mkForce vs mkForce = conflito). `initialPassword`
    nao se aplicou no live: usar `users.users.root.password`.
22. Duas sessoes no mesmo working tree: **nunca `git add -A`**; stage caminhos
    explicitos. A outra sessao pode commit/push e arrastar ficheiros teus.
23. Ring armado = **duas chaves**: `armRing=true` tem de armar o helper root
    **e** o daemon (`NOSD_NIX_RING=1` nos dois via `environment` do unit).
    Armar só o helper = `NIX_RING_DISABLED` invisível no nosd — apanhado só
    quando o VM test corre o fluxo ARMADO (`nixaios-ring`, stub rebuild).
24. Toolbox do helper root: **comando usado no script tem de estar no path
    do UNIT** (`grep` não faz parte de coreutils; sem `pkgs.gnugrep` o apply
    morre 127 ANTES da fase 1). O caminho desarmado sai antes do grep — bug
    latente só apareceu com teste armed ponta-a-ponta.
25. Parse de JSON no test driver: `serde_json::to_string_pretty` imprime
    `"key": "value"` (espaço após `:`) — split com `'"key":"'` quebra;
    usar split por `'"key":'` + index de aspas, ou `json.loads` (python3).
26. Depois de editar `.nix` com `${VAR}` dentro de `writeShellScript ''…''`:
    rodar `nix-instantiate --parse` em cada módulo — `${VAR}` sem escape
    (`''${VAR}`) é interpolação Nix e a Pancada só aparece no build/VM.
27. PowerShell: `cargo … 2>&1 | Select-String` gera `NativeCommandError`
    stderr + **falso exit 1**; validar exit real com `… 2>&1 | Out-Null;
    $LASTEXITCODE` (leções de CI local, não arrastar por falso-positivo).
28. Firstboot interactivo no metal tem de correr no **tty1**: o getty
    serial (`console=ttyS0`) pode ganhar o flock e as perguntas ficam
    invisíveis (ecrã parado, sem GUI). O wrapper só dispara firstboot em
    ttyS*/hvc/console com `NOSD_FIRSTBOOT_AUTO=1` (ou `ASK=1`).
    `console=tty0` mantém o boot visível; respostas persistem em
    `firstboot.conf`/`wifi.conf` no pendrive (2º+ boot auto, 10 s para
    reconfigurar); `models/` é o atalho para o GGUF (seed antes do nosd).
29. **A superfície da CLI é código de runtime**: `nosctl --help` morreu com
    stack overflow (0xC00000FD) depois de crescer para ~80 subcomandos — o
    `clap` constrói a árvore na stack e o Windows dá 1 MiB em debug. Fix:
    `main()` corre o runtime numa thread com `stack_size(16 MiB)`. Unit test de
    `render_*` nunca apanha isto; o smoke tem de chamar `--help`.
30. **Triage por substring mente**: procurar `absent` no JSON de `doctor` deu 5
    falsos `EngineAbsent` (`llama_server:"absent"` é posture normal com
    `cpu-echo`). Warn tem de ser por campo/subsistema, nunca por `contains`.
31. **Energia sem `scope` mente**: RAPL mede o package da CPU, NVML mede a GPU;
    nenhum mede a máquina. Custo local = **limite inferior** excepto
    `tdp_estimate` (`machine_estimate`). `absent` = 0 J, nunca estimativa.
32. **Superfície dinâmica (MCP)**: piso IPC `NONE` + caps **da tool** no handler
    é o padrão seguro (registry server-authored). Não abrir
    `http_lab::allowed_methods` sem teste de contrato.
33. **Percentagem sem denominador é uma afirmação que ninguém verifica.**
    Denominador vazio ⇒ `percent:null` + `basis=no_observations`, **nunca 0%**
    (`nos-core::reported_rate`, ideia do oh-my-hermes). `meets_target` falha o
    não medido. Contagens sim; dinheiro não (custo nuvem é contrafactual).
34. **Loop-breaker = fingerprint + recusa, nunca revert.** O
    `git checkout .` do eco-tokens apaga trabalho por teste flaky. Em `nosd`:
    `LoopGuard` (BLAKE3) recusa com `loop_detected` e `reverted:false`.
35. **Allowlist cross-platform: filtrar por `is_absolute()` em runtime.** No
    Linux, `C:\Windows\System32\nvidia-smi.exe` é um caminho **relativo** (1
    componente) — um `stat`/`Command` resolve-o contra o cwd do daemon. O
    `nix build` em WSL apanhou isto; o host Windows não (lá o mesmo caminho é
    absoluto e os testes passavam). Listas de candidatos a binários têm de
    largar os candidatos "estrangeiros" da plataforma.
36. **Commit leva `Tested:` / `Not-tested:`** no corpo (doutrina oh-my-hermes).
    O `git log` é o ledger barato: sem o que **não** foi testado, "verde" vira
    folclore e a sessão seguinte não sabe onde olhar.
37. **Animação de `opacity` em painel do kiosk prende layers**: no Chromium/cage
    os painéis ficavam a ~20% (texto/fundo escuros) mesmo com `opacity:1`
    computado. Entradas de HUD animam por `transform`, nunca `opacity`.
38. **O breaker do eco-tokens pode ter hash vazio** (o Python do MCP não vê
    `git` no PATH → SHA256("")). O reset é decisão humana (editar
    `.eco/state.json`/`update_task_status`), **nunca** `git checkout`/`stash`.
    O `mtime` do Windows pode ficar atrás do binário: forçar `cargo clean -p`.
39. **Duas instâncias nosd na mesma máquina para testar mesh/HUD**: portas UDP
    distintas + unicast (`19000↔19001`), `--node-id` 15/16, data-dirs
    separados. Com `pinned=1`/`TOFU SETTLED` a janela 06 aparece; matar a B
    esconde-a no poll seguinte.
40. **Python `write_text` no Windows grava CRLF**: reescrever `flake.nix` assim
    mete `\r\n` dentro de strings `''…''` (ex.: `postFixup`) e o stdenv morre
    no fim do build com `$'\r': command not found` (fixupPhase). O blob git é
    LF, mas o Nix usa a árvore de trabalho quando está *dirty*; converter o
    ficheiro para LF (`perl -pi -e 's/\r\n/\n/g' flake.nix`) antes de
    `nix build`. Não é falha do Rust nem do modelo.
41. **`DeviceAllow` resolvem aliases por `/proc/devices` (Linux-only, unit) —
    lista NVIDIA de sítio nenhum passou em metal**: `char-nvidia` cobre
    `/dev/nvidiaN` **e** `/dev/nvidiactl` (major 195). `char-nvidia-frontend`
    é módulo vGPU/Xen e `char-nvidia-ctl` não é alias: sob política `closed`
    implícita o systemd **ignora-os em silêncio** e a CUDA morre a abrir
    `/dev/nvidia0` (Permission denied → ENODEV). Host Windows não decide
    unit (enforcement é systemd/kernel) — gate só existe no lado Nix:
    `checks.hw-cuda-deviceallow` no flake (eval + grep; inclui unit do
    `nosd-engine` supervisionado). Apanhar a 2.ª/3.ª lista igual no MESMO
    ficheiro: `grep -n 'char-nvidia-' modules/*.nix images/*.nix` antes de
    declarar o fix feito — o 1.º round de patch pegou **3 de 4 listas**
    (o `nosd-engine` supervisionado só ficou coberto à 2.ª passagem, quando
    o check passou a avaliar-o). Metal: confirmar
    `journalctl -u nosd-engine-s2 -b | grep -iE 'nvidia0|Permission denied'`
    vazio. Ver `docs/evidence/HWCUDA_DEVICEALLOW_20261030.md`.

## Comandos

HW (um só pendrive): `nix build .#iso-hw` -> `scripts/pack-hw-usb.sh <iso> <img>`
(ISO + FAT32 `NIXAIOSLOG` no mesmo `.img`) -> gravar em DD -> `models/` +
`wifi.conf` -> first boot interactivo no tty1 (Q1-Q5) antes do GUI. O seed
copia o GGUF para `/var/lib/nosd/models` antes do nosd; llama.cpp esta
incluido na iso-hw (a ISO de release nao o tem). Ver
[`docs/HW_TEST.md`](docs/HW_TEST.md).

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
  pk-bound, Master/Worker.
- **M5.1 falhas**: `mesh-3vm-faults` (3 VMs) OBSERVED: convergencia nos 3,
  netem 200ms, perda 100%, `kill -9` + replay `NSGC`, particao `ip link down`.
  Encontrou e fechou 2 bugs (VV do CRDT por chave; seq do HB ao reiniciar).
  **Frota multi-host/NAT/Wi-Fi, TTL de peers e Merkle/bloom** pendentes.
- **M5.1 demo ISO**: `iso-demo` (NUNCA release) prova a mesh em 3 QEMU:
  `mesh_pinned: 2` nos 3, crdt 1->2->3 e job 1->3 via SSH, com VNC por no.
  IP derivado do MAC, firewall aberto, root/nixaios - so na demo; a ISO de
  release continua sem identidade/mesh por omissao. Ver `docs/DEMO_3VM.md`.
  Infra: `socket,listen` so aceita 1 ligacao e mcast nao passa no WSL -> relay
  UDP userspace; o screendump do virtio-gpu saiu preto com o cage active
  (HUD observado via VNC).
- **Tranche 0.6.0-alpha.13 (in-tree, HARDENING+SIMBIOSE)**: G0 divida fechada
  (SKILL_PUSH desbloqueado - payload de skill tem teto proprio 768 KiB vs 8
  KiB de chat; score de skills clamp 0-100; `hitl_confirmed` fora do
  audio.volume - cap HITL da identidade decide; `NOSD_ENGINE_DIR` morto
  removido; `services.nosd.package` propagado a early/firstboot/hotswap;
  StartLimit no UNIT nosd; 1777 do data-dir so pre-transmute). G1: ancora do
  CRDT **assinada** (`NSGANCHOR bytes lines` + sig; sem-assinatura com
  identidade = recusa), pins TOFU persistidos (`keys/pins.dat`, HB+jobs),
  seq do HB persistido (`keys/hb.seq`, arranque max(epoch_ms, persistido+1)),
  Landlock AtomicU32. G2: `services.nosdEngine.supervise` (motor como
  servico systemd + nosd por HTTP externo; default OFF). G3: `nix.propose
  fields={...}` template tipado + apply root **two-phase** (copy+build-vm+
  `test`; switch so com `nix.confirm`, TTL 600s) + `gate --strict`. Check
  **nixaios-ring** (helper armado com stub rebuild) pegou 3 bugs latentes
  (gnugrep no path; split do pretty JSON; armar daemon). **Brain
  self-upgrade**: `brain.plan`/`brain.accept` com tiers pinados (0.8B Q8_0 /
  4B / 9B dGPU, sha256 LFS + bytes exactos), custo/beneficio honesto na card
  HITL (`brain.upgrade`, tier4 exige ring), sha verificado no fetch (diverge
  = apaga), gancho pos-boot `NOSD_BRAIN_PROPOSE=1` (anti-spam 24h via CRDT).
  Host-level verde (clippy/testes/workspace). Residual: **sem OBSERVED**
  (VM re-prova/Q-03 externo/metal); detalhe em
  `docs/memory/SESSION_20261029_TRANCHE_BRAIN.md`.
- **Tranche 0.6.0-alpha.14 (in-tree, PLANO VAMPIRO V0–V7)**: absorção selectiva
  do OpenJarvis **sem** runtime Python/Node — `nosctl doctor|config-show|
  energy|trace|pending|mcp-list|mcp-call|skill-import|dash`; `nos-core::
  {metrics,cost}`; `nos-observe::energy` (RAPL/NVML/TDP com `scope`);
  `cognitive.trace` redigido (sem notas/PII); `ui.pending` (tier+TTL, sem
  nonce); crate `nos-mcp` (in-process, caps por tool, side-effect exige HITL);
  `skill.import` (SKILL.md declarativo, allowlist `log|memory:read|
  aios_ai.infer|net:read`); HUD Slint com energia/fila/OPAR. Host **OBSERVED**
  (`scripts/smoke-vampire.sh` PASS; 336 testes; clippy/fmt verdes). Residual:
  **sem** nix build/QEMU/metal, RAPL real AWAITING_HW, MCP stdio/SSE NOT_DONE.
  Ver `docs/evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md` e
  `docs/memory/SESSION_20261029_VAMPIRE.md`.
- **Tranche 0.6.0-alpha.15 (in-tree, V8 — OMH + eco-tokens)**: `nosmcp` (MCP
  stdio **read-only**: health/score/energy/pending/trace); `nos-core::
  reported_rate` (denominador vazio = `percent:null`); `nos-core::
  trim_diagnostic` (ANSI + 5/15 + cap 1000) usado nos erros de chat e no
  doctor; `doctor` com `reported|verified|unmeasured`; prompt
  `## Persona`→`## Memory recall`→`## User` (prefixo KV estável); `LoopGuard`
  no `self.heal` (recusa, sem revert). D1: `nvidia-smi` por allowlist
  absoluta + `docs/SPAWN_INVENTORY.md`; `path` do unit com `systemd`/`coreutils`.
  Host **OBSERVED** (`smoke-nosmcp.sh` + `smoke-vampire.sh` PASS; 363 testes).
  **Linux**: `nix build .#nosd` PASS (363 testes na derivação), ISO
  `NIXAIOS051` construída (sha256 `4c9c55a6…`), QEMU `--stage MQEMU` fails=0
  (`nosd_start_ok`, cage ACTIVE, firstboot DONE). Residual: metal AWAITING_HW,
  MCP resources/prompts, prompt-eval não medido.
  Ver `docs/evidence/V8_OMH_ECO_0.6.0-alpha.15.md`.
- **Tranche 0.6.0-alpha.16 (in-tree, V9 — DAG + kiosk)**: `nos-core::dag`
  (camadas topológicas + **isolamento de write-scope** por segmento; ciclo/
  dep desconhecida/duplicado = erro; 256 nós; determinístico) exposto por
  `dag.plan` (NONE, **não executa**) e `nosctl dag-plan <file>`; kiosk
  (`KIOSK_METHODS`) passa a expor `observe.energy` e `ui.pending` com teste de
  contrato (caps NONE/INFER/HITL; 17 métodos de escrita recusados); convenção
  `Tested:`/`Not-tested:` (lição 36). **Execução** do DAG (job.submit/factory)
  fica para V10. Host OBSERVED (`dag-plan` diamante/conflito, 374 testes,
  smokes V0–V8 verdes). Ver `docs/evidence/V9_DAG_KIOSK_0.6.0-alpha.16.md`.
- **Tranche 0.7.0-alpha.2 (in-tree, ORB UI + HW cinematic)**: HUD web
  voz-primeiro (mic antes da câmara), CC só STT/TTS em modo voz (texto quando a
  voz está off), HITL modal com tier/campos/hold, mesh `health.mesh_nodes` com
  mini-orbs por papel + trilhos de dados bidireccionais, `observe.energy.split`
  (CPU RAPL/GPU NVML) + termal hwmon advisory, e `iso-hw` com
  `uiMode=cinematic` (Chromium `--kiosk`) e todos os GGUFs pinados. alpha.2:
  tier **4B Q6_K** para 6 GB (escolhido antes do Q4) e contexto automático
  (`llama-server --fit on --fit-ctx`, sem `-c` fixo). Host
  OBSERVED (2 instâncias N15/N16, screenshot `docs/evidence/artifacts/
  orb_ui_mesh_2nodes.jpeg`, verify PASS). `nix build .#iso-hw` PASS: ISO
  15 946 315 776 B, sha256 `c5d7cb80…f67f`; `.img` único 20 241 711 104 B,
  sha256 `4662a340…9225` (ISO + FAT32 4G; 5 GGUFs). Metal/QEMU AWAITING_HW. Ver
  `docs/ORB_UI.md` e `docs/evidence/ORB_UI_0.7.0-alpha.2.md`.
- **Metal CUDA 0.7.0-alpha.2 (in-tree; fixes pós-alpha.2)**: `iso-hw-cuda` com
  driver NVIDIA + `llama-cpp-cuda`; S1 0.8B Q4 em CPU e S2 4B **Q6_K** em CUDA
  na 8082 (`--fit on --fit-ctx 2048 --reasoning off`, sem `-c`/`-ngl` fixos).
  Chromium no cage: `NIXOS_OZONE_WL=1` + `--ozone-platform=wayland` (sem isto
  tentava X11 e caía para Slint), `security.chromiumSuidSandbox` e retry
  `--disable-gpu`. ISO `4f5ce467…` (17,2 GB), img 21 515 730 944 B sha
  `f7da2843…` (tail-sha == ISO; inclui `char-nvidia`/DeviceAllow). **Metal
  AWAITING_HW** (drivers/GPU).
  Ver `docs/evidence/hw/2026-10-31_0140Z/README.md`.
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
- **M8.1e log autenticado**: registo = json+tab+sig Ed25519 do no; replay
  recusa registo invalido/sem sig (com identidade). Vale nos dois formatos.
- **M8.1d anti-rollback**: ancora (`seg.anchor`/`crdt.anchor`, `<bytes> <linhas>`)
  ao lado do log; gravada ao abrir, ao compactar (antes do rename) e a cada 32
  linhas. Log em disco mais pequeno do que a ancora = recusa arrancar
  (fail-closed). **Falta**: protecao contra quem escreve no data-dir (pode repor
  log e ancora); janela de ate 31 linhas.
- **M8.1d backend**: log em `nsgdb/crdt/seg.bin` (`NSGC` length-prefixed,
  registo `json\tbase64(sig)`) e **default**; `NOSD_CRDT_BACKEND=jsonl` mantem o
  legado, com migracao one-shot (`crdt.jsonl.migrated`). **Falta**:
  indice/consulta (replay completo ao abrir), migracao inversa e o motor
  neural-sgdb real (sem BQ/semantico).
- **M8.1c compactacao**: runtime (60 ticks) + fsync do diretorio apos rename;
  vale nos dois formatos.
- **M8.1b durabilidade**: log append + replay, fail-closed ao abrir. Desde
  M8.1d o formato default e o segmento `NSGC`; sem fsync por op.
- **M8.1 delta**: `crdt.sync` = pull por VV + push do delta (2 trocas cifradas).
  Sem bloom/Merkle; estado em memoria (replay no arranque); sem compactacao.
- **M6.1 Noise**: link de jobs/estado cifrado (XX + BIND Ed25519 contra a chave
  fixada). Sem modo plaintext. Sem revogacao/rotacao; HB UDP continua em claro.
- **M9 skills**: wasmi frugal (fuel 100k/heap 256 KiB/zero caps),
  `skill.install|list|run`, registo `skill/<n>/active` no CRDT. Local apenas
  (sem SKILL_PUSH); fnv como id, sem BLAKE3 nem quota.
- **M7 jobs**: `job.submit` + listener TCP `NOSD_JOB_BIND` com frames
  assinados; worker tem de ser peer fixado. Sem Noise/fila/quota por peer.
  Check `mesh-2vm-jobs` PASS. Ver `docs/SKYNET_DISTRIBUTED_PLAN.md`.
- **M4/M5**: AWAITING_HW.
- **UI híbrida JARBAS**: `jarbasCage.uiMode = auto|slint|cinematic` (default
  `slint`). `auto` escolhe Chromium só com
  `MemAvailable >= chromiumMinAvailableMiB` **e** `chromiumGpuValidated=true`;
  falha do Chromium (lab em baixo / renderer a sair) cai para Slint. O kiosk
  Chromium usa o bridge HTTP **sem `ipc.token`**: chave bootstrap 0600 → cookie
  `HttpOnly`, perfil `AuthContext::JarbasUi`, origem exacta, allow-list de
  métodos; `GET /transmute` = 403. Ver `docs/SECURITY.md` (HTTP lab) e
  `docs/API.md`. Testado em bancada (unit + ponta-a-ponta com daemon local);
  **ainda não observado em QEMU/metal**. Desde a tranche ORB UI
  (`docs/ORB_UI.md`), `images/hw.nix` força `uiMode = "cinematic"` no HW real
  (Chromium `--kiosk` fullscreen, fallback Slint); ISO de release/QEMU
  continuam `slint`.

## Evidência 0.7.0-alpha.2

`docs/evidence/ORB_UI_0.7.0-alpha.2.md`, `docs/evidence/artifacts/orb_ui_mesh_2nodes.jpeg`.

## Evidência 0.6.0-alpha.16

`docs/evidence/V9_DAG_KIOSK_0.6.0-alpha.16.md`, `docs/evidence/V8_OMH_ECO_0.6.0-alpha.15.md`, `docs/evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md`, `docs/evidence/QWEN35_0.8B_0.6.0-alpha.12.md`, `docs/RELEASE_0.6.0-alpha.11.md`, `docs/evidence/QEMU_0.6.0-alpha.11.md`, `docs/evidence/M5.1_3VM_FAULTS_0.6.0-alpha.10.md`, `docs/RELEASE_0.6.0-alpha.10.md`, `docs/evidence/QEMU_0.6.0-alpha.10.md`, `docs/evidence/M8.1D_NSGDB_BACKEND_0.6.0-alpha.9.md`, `docs/evidence/M8.1D_CRDT_ANCHOR_0.6.0-alpha.9.md`, `docs/evidence/S2C_SIGNED_PUSH_E2E_0.6.0-alpha.9.md`, `docs/evidence/S2C_REVOKE_0.6.0-alpha.9.md`, `docs/evidence/CLI_SKILL_AUTH_0.6.0-alpha.9.md`, `docs/RELEASE_0.6.0-alpha.9.md`, `docs/evidence/S2B_SKILL_CAPS_0.6.0-alpha.8.md`, `docs/evidence/S2A_SKILL_PUBLISHER_0.6.0-alpha.8.md`, `docs/evidence/S1_SKILL_TRUST_0.6.0-alpha.8.md`, `docs/evidence/M8.1e_CRDT_LOG_AUTH_0.6.0-alpha.8.md`, `docs/evidence/M8.1c_CRDT_COMPACT_0.6.0-alpha.8.md`, `docs/evidence/M8.1b_CRDT_LOG_0.6.0-alpha.8.md`, `docs/evidence/M8.1_DELTA_0.6.0-alpha.8.md`, `docs/evidence/M9.1_SKILL_PUSH_0.6.0-alpha.8.md`, `docs/evidence/M9_SKILLS_0.6.0-alpha.8.md`, `docs/evidence/M7_JOBS_0.6.0-alpha.8.md`, `docs/evidence/M6.1_NOISE_0.6.0-alpha.8.md`, `docs/evidence/M6_IDENTITY_0.6.0-alpha.8.md`, `docs/evidence/HITL_0.6.0-alpha.8.md`, `docs/RELEASE_0.6.0-alpha.7.md`, `docs/evidence/NIX_RING_0.6.0-alpha.7.md`, `docs/RELEASE_0.6.0-alpha.6.md`, `docs/evidence/SECURITY_S08_S09_0.6.0-alpha.6.md`, `docs/RELEASE_0.6.0-alpha.5.md`, `docs/evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md`, `docs/evidence/CAGE_SLINT_0.6.0-alpha.4.md`, `docs/evidence/VOICE_0.6.0-alpha.2.md`, `docs/evidence/VM_GGUF_0.6.0-alpha.2.md`, `docs/evidence/QEMU_0.6.0-alpha.2.md`.
