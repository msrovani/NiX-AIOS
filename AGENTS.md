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

Versão corrente: **0.7.0-alpha.6** (in-tree, HUD C1–C6; última ISO publicada `0.7.0-alpha.5`). Volid ISO: **`NIXAIOS051`**.
Memória cognitiva: neural-sgdb via `MemoryGovernor` — ver [`docs/MEMORY_NSGDB.md`](docs/MEMORY_NSGDB.md) + [`docs/NSGDB_VERSION.md`](docs/NSGDB_VERSION.md).
Closeout: [`docs/evidence/FORUM_CLOSEOUT_0.7.0-alpha.5.md`](docs/evidence/FORUM_CLOSEOUT_0.7.0-alpha.5.md) — próximo P0 = colheita metal Core7.

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
42. **`tr -d '\r'` em massa apaga a letra `r`**: `return`→`retun`, `warn`→`wan`,
    `kernel`→`kenel`. Em Windows use conversão CRLF→LF (`perl -pi -e 's/\r\n/\n/g'`
    ou editor), nunca strip cego de todos os `\r`/`r`.
43. **BOM UTF-8 no `flake.nix`**: o eval Nix falha à cabeça; strip BOM antes do
    `nix build` (e `git add` o ficheiro limpo).
44. **`pack-hw-usb.sh` no VHDX WSL**: `dd` WORK→OUT no mesmo disco **duplica**
    ~20 GiB e pode encher C: (RO remount / `ERROR_DISK_FULL`). Same filesystem →
    `mv` do WORK_IMG; só `dd` streaming para 9p/drvfs. Preferir OUT em ext4 WSL
    (`~/….img`), não em `/mnt/c` com pouco livre.
45. **Hybrid `.img` no Windows**: partições EFI+FAT32 do append ficam muitas
    vezes **Offline** (D/E “dispositivo não está pronto”). Online em
    `diskmgmt.msc` ou `wsl --mount \\.\PHYSICALDRIVEn --partition N` (admin)
    antes de ler `NIXAIOSLOG`/`boots/`. Rufus elevado: Computer Use não atravessa
    UIPI — START/DD é manual ou script admin.
46. **JARBAS “não altera Nix” com ring desarmado é correcto**: `armRing=false`
    → `NIX_RING_DISABLED`. Auto-melhoria = skill/tool `nix.propose` → HITL →
    `nix.confirm` (helper root); nunca `nixos-rebuild` no chat. Lab:
    `armRing=true` só com S-04 + ADR + check `nixaios-ring`.
47. **Matar o wrapper não mata o filho**: `taskkill //F //PID <bash>` de um
    watcher deixou o `python` vivo — dois pollers escreveram no mesmo log e no
    mesmo ledger durante ~30 min (ciclos repetidos, linhas truncadas; o
    `forum-log.py` só deduplica por `id`, não por conteúdo). Parar = matar o
    **PID do intérprete** (`Get-CimInstance Win32_Process` filtrado por
    `CommandLine -like '*<script>'`), ou `taskkill //F //T` a partir de um PID
    que ainda exista. `//PID` do bash **não** é prova de paragem.
48. **Hash do modelo pinado prova-se com `nix build`, não com "confiar no
    post"**: GGUF de terceiro novo (ex.: quantização do edito Astra
    2026-11-04) entra como `fetchurl` com SRI do LFS sha256; o `nix build`
    que bate é a prova real dos bytes (`8wv2737h…`). Converter SRI
    (`python -c "base64(bytes.fromhex(hex))"`) e comparar `sha256sum` —
    lição 37: ler o README não é VERIFIED.
49. **Ganho de modelo editado é do protocolo de medição, não do modelo em
    si**: o edito da Astra (Weight-Space Programming) é provado em
    safetensors bf16, **thinking ON**, 32k ctx (MATH-500 7.9→48.1).
    No NixAIOS o boot corre `--reasoning off` + fit-ctx 2048 + Q4_K_M —
    outro protocolo: documentar SEMPRE os caveats junto do pin
    (`flake.nix`), guardar rollback sem edito (`-base`), e quantização de
    publisher novo não faz claims de qualidade — só o hash fala.

50. **FAT dentro de `.img` híbrido não lê com offset no mtools**:
    `mdir -i img@@8325596288 ::` = `non DOS media` (a ISO+partição híbrida
    engana o mtools). Prova da FAT32 = `losetup -P` + `mount -o ro` no
    **`wsl -u root`** (o `sudo` do host pede password; root WSL não), e
    `losetup -d` sempre no fim — loop da sessão anterior ficou pendurado
    em `/dev/loop0`.
51. **Job WSL lançado da sessão morre sem rasto**: o log em `/tmp` meio
    caminho desaparece com o processo (não há `ps` de sobra, não há log).
    Tarefa longa = script bash (`_tmp_*.sh` no repo, apagar depois)
    com log em **`$HOME`** (não `/tmp`), correndo em background.
52. **"A ISO contém o modelo X" prova-se com `unsquashfs -l`, não com o
    post**: `nix shell nixpkgs#xorriso nixpkgs#squashfsTools` →
    `xorriso -osirrox on -indev $ISO -extract /nix-store.squashfs` +
    `unsquashfs -l | grep` com contagem (`G-matches: 409`). A extractor
    do squashfs da live ainda está a copiar a 4 GiB/min, e só o
    listing da árvore extraída fala por si (compare store paths com os
    pins do `flake.nix`).

53. **Pendrive `NIXAIOSLOG` sem letra lê-se no dispositivo cru, não com
    `mount`**: a FAT32 do `.img` híbrido fica Offline/sem letra no Windows
    (lição 45) e `mtools` com offset mente (lição 50). `Get-Partition` dá o
    offset (`8323596288` no layout do `pack-hw-usb.sh`); abrir
    `\\.\PhysicalDriveN` exige admin; com parser FAT32 próprio
    (BPB→FAT→clusters, read-only) o bundle sai inteiro e com sha256. Foi
    assim que a colheita de 3 Nov saiu de `AWAITING_HW` para
    `docs/evidence/hw/2026-11-03_2346Z/` (41 ficheiros). Script:
    `scripts/extract-usb-logs.py` → `logs/`.

54. **`is_file()` não prova conteúdo — o temporário já existe antes do
    escritor**: `tempfile` cria o ficheiro, logo `out.is_file()` é verdade
    mesmo quando o sintetizador falhou; um WAV de 0 B passava por bom e o
    fallback (espeak) nunca disparava. Validar tamanho/magic
    (`len() > 44`), não existência — vale para qualquer ficheiro de saída.

55. **`ok:true` com conteúdo vazio é o `0%` sem denominador da voz**: o
    whisper devolve `Ok("")` quando não há fala e o handler respondia
    `ok:true, text:""`; o ORB marcava STT pronto sem ter ouvido nada.
    Vazio = erro explícito (`nos_voice::transcript_or_error`), com
    `empty:true` no resultado.

56. **Flag explícita anula heurística — e o restart esconde o estrago**:
    `-ngl 99` desliga o `--fit on` que devia escolher o que cabe na VRAM;
    num 4B Q6 em 6 GiB já ocupados deu `cudaMalloc: out of memory` e
    `nosd-engine-s2.service: Failed with result 'exit-code'`, recuperado só
    no restart (já com fit). Quem lê o `health` final vê "8082 a
    responder" e não vê o arranque falhado: ler `journal-follow`, não o
    retrato.

57. **Instrumento honesto: PATH não é serviço, default não é formato**:
    A2 dizia `pipewire_absent` só porque `wpctl` não estava no PATH do
    probe, com `pipewire-0` no runtime dir; A4 gravava 8000 Hz (default do
    ALSA) quando o whisper só aceita 16 kHz — dois falsos negativos sobre um
    mic que funcionava (`32044 B`). Medir o objecto (socket/card) e, quando
    o consumidor exige formato, o probe tem de o declarar (`-r 16000 -c 1`).

58. **`ignoreCollisions = true` resolve por `meta.priority` — e o nixpkgs já
    reservou 4 para o `nvidia_x11`**: `register_env`/`system-path` usa
    `buildEnv { ignoreCollisions = true }`, prioridade **menor** ganha a
    colisão de `bin/nvidia-smi`. O `nvidia-bin-elf-pf-w` (default 5) ficava
    no store mas **perdia** o symlink do perfil, que continuava a ser o
    binário por corrigir — o fix estava certo e o metal crashava na mesma
    (2026-11-03: `hw-info.log` executou `/run/current-system/sw/bin/
    nvidia-smi`). Quem substitui um binário de um pacote alheio tem de usar
    `lib.hiPrio` (priority < 4) e provar que ganhou, não só existir no store.

59. **Flags certas ≠ binário carrega**: `segfault at 401488 … error 7 in
    ld-linux` é o `ld.so` a escrever `DT_DEBUG.d_ptr`
    (`0x401488 = .dynamic(0x4013a0) + 0xe8`) num `PT_LOAD` só `PF_R` depois
    do patchelf; `error 7` = escrita em página presente/user read-only. O
    check que só inspecciona `p_flags` passou com o binário a crashar. A
    prova é **correr** o ELF sob o `ld.so` do alvo:
    `${glibc.out}/lib/ld-linux-x86-64.so.2 --library-path … --help` com
    rc=0 (host: original 139 → corrigido 0; `glibc` default output é `bin`
    e não traz o loader — usar `.out`). Check `nvidia-smi-loads`.

60. **`Unable to get gpu adapter` não é a GPU do compositor**: no Chromium essa
    linha vem de `services/on_device_model/ml/gpu_blocklist.cc` — o probe do
    modelo on-device ("Gemini Nano") para features de IA que o kiosk não usa.
    O render morreu por **Vulkan com `--ozone-platform=wayland`**
    (`wayland_surface_factory.cc:249`) e o retry `--disable-gpu` escondeu-o.
    Ler o ficheiro que emite a linha, não a substring (`adapter`). Kiosk:
    desligar o que não se usa (on-device model/optimization hints) **e** a
    telefonia (`--disable-background-networking`; o log tinha GCM a responder
    `PHONE_REGISTRATION_ERROR`).

61. **ELF de distro com `dontFixup` = ENOENT enganador**: o tarball do piper tem
    `PT_INTERP=/lib64/ld-linux-x86-64.so.2`, que não existe no NixOS. O ficheiro
    existe, passa `-x`, e `Command::new(x).spawn()?` devolve
    `io: No such file or directory (os error 2)` — irmão da lição 59: `-x` não
    prova que um ELF carrega. Fix: `patchelf --set-interpreter
    ${glibc.out}/lib/ld-linux-x86-64.so.2` **e** wrapper com `LD_LIBRARY_PATH`
    (`$out`, `<glibc>/lib`, `gcc cc.lib/lib`) — `DT_RUNPATH` não chega às
    dependências indirectas. Prova = correr (`--help` rc=0 + uma síntese real).

62. **Auto-login em `ttyS0` cega o gate MQEMU**: na `iso-hw` o `nixos login:
    nixos (início de sessão automático)` toma a consola **antes** de o systemd
    imprimir `Started Neural-OS Daemon`; o gate só lê o log de série, logo dá
    `PASS` nos marcadores de kernel e `SKIP detail=nosd start status unclear`.
    As ISOs de release (sem auto-login) continuam a dar `nosd_start_ok` — a
    cegueira é da ISO, não do produto. Quem ler isto vai ao `journalctl -b -u
    nosd` (lição 63). **Nunca** acrescentar a linha ao log para o gate passar:
    é o mesmo pecado do `grep NixOS` que casa o menu.
63. **`run-iso-qemu-wsl.sh` liga a série a `stdio`**: com `nohup`/pipeline o
    stdio é `/dev/null` e o guest fica **inaudível** (não há por onde perguntar).
    Para interrogar: `-serial unix:$HOME/ser.sock,server,nowait -display none
    -monitor none` + cliente Python que faz `connect` ao socket e envia
    comandos. Foi assim que saíram `systemctl is-active nosd`, o journal e
    `/proc/<pid>/cmdline` de dentro da imagem.
64. **Sem grupo `kvm` o QEMU cai em TCG e `is-active` cedo mente**: `/dev/kvm` é
    `root:kvm`; sem o grupo o script escolhe `-cpu qemu64` e um boot de ISO
    leva ~6–10 min. E a primeira leitura deu `inactive` para um daemon que
    minutos depois estava `active (running) since 22:53:07` (ExecStartPre +
    firstboot atrasam). Ler `status` com timestamps, nunca um retrato — a
    doutrina do `journal-follow` (lição 56) também vale para o guest.
65. **O host de 7 GiB mete tecto no que o guest pode provar**: guest de 4 GiB e
    o 4B Q6_K tem 5,4 GB — prova de modelo grande **não** sai do QEMU neste
    portátil. Duas provas, dois sítios: imagem (marcadores + argv do 0.8B no
    guest) e processo real (binário novo + GGUF real no host). Não forçar o 4B
    num guest que não o segura; dizê-lo no `Not-tested`.
66. **Prova de que a correcção está na imagem = string que só o código novo
    emite**: boots de builds diferentes parecem iguais. Ancorar em marcadores
    únicos (`suggest_ngl=0 (hint only — --fit decides`, `hub slot=active
    conflict keep=`, `spawned … ngl=0 thermal_cap=None`) e contar flags em
    `/proc/<pid>/cmdline` com `tr '\0' '\n' | grep -c '^-ngl$'` — nunca por
    substring (lição 30).
67. **`inactive (dead)` com `-- No entries --` é `ConditionPathExists`, não GPU**:
    o `jarbas-cage` só arranca depois de `/var/lib/nosd/gui-start` (hotswap
    pós-Q5 / persist-check), e num guest com auto-login em `ttyS0` esse fluxo
    nunca corre — o unit é saltado em silêncio (zero linhas de journal, seatd
    active, `/dev/dri` presente). Diagnóstico: `systemctl show -p ConditionResult`,
    `ls /var/lib/nosd/{gui-start,firstboot.done,aios-mode}` e o journal a dar
    `-- No entries --`. Para provar a UI sem o firstboot: `touch gui-start` +
    `systemctl start jarbas-cage`. Sem GPU o wlroots cai em **pixman** (EGL
    "Software rendering detected", Vulkan `ERROR_INITIALIZATION_FAILED`) e
    desenha na mesma no conector `Virtual-1` — provado por `screendump`.
    Duas armadilhas do bancada: a série em socket aceita **um** cliente (o
    segundo `connect` dá `Transport endpoint is not connected` e os comandos
    são ecoados sem executar), e **`quit` no monitor fecha a VM** — um
    `screendump` seguido de `quit` mata o guest e a captura seguinte não existe.
68. **`tail -c <ISO_SIZE> img | sha256sum` não é prova do embed**: no `.img`
    híbrido o ISO fica no **offset 0** e o `sfdisk` acrescenta a FAT no fim,
    reescrevendo a MBR (differ no byte 480 — entrada de partição). O tail pega
    só a cauda do próprio ISO, e o hash nunca bate por razão nenhuma. A prova é
    `cmp -n $((ISO_SIZE-1MiB)) -i 1048576:1048576 img iso` (idêntico), mais
    `sfdisk -l` para o layout. Para empacotar no WSL, `mkfs.vfat` não está no
    PATH: `nix shell nixpkgs#dosfstools nixpkgs#util-linux --command bash -c …`.
69. **`--fit-ctx` é o piso do `--fit`, não o tecto — e o contexto prova-se no
    `/props`**: o help do build 11429 diz `minimum ctx size that can be set by
    --fit option, default: 4096`; um `n_ctx` acima disso (o nativo do modelo)
    não é violação do cap. Não julgar contexto por linha de log nem por
    substring: `GET /props` do **próprio motor** é a fonte do valor efectivo.
    No S1 managed a porta é dinâmica (`pick_port`) e o `/props` exige
    `Authorization: Bearer <--api-key do argv>` — a chave fica em memória e o
    `argv` que vai para o bundle sai com `<redacted>`, senão o pendrive leva a
    chave (ainda que de loopback). Provado no host com o binário pinado:
    `props_http=200`, `n_ctx=262144`, `0` ocorrências da chave no bundle.

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

- **Tranche inferência/VRAM/voz/fábrica (2026-11-06, in-tree, publicada)**: plano
  `docs/PLAN_INFERENCE_VRAM_VOZ_FABRICA.md` fases 1–7 feitas — `8f08cbd` F1,
  `d7b7884` F2, `575095a` F3, `7a62319` F4, `9ddfac7` F5, `c3ce2a4` F6,
  `d175b0c` F7, docs `b54a5f9`/`86e09b2`; gate host **1037** testes.
  QEMU **OBSERVED** (`docs/evidence/QEMU_INFERENCE_2026-11-06.md`): ISO
  `iso-hw` de `main` sha `dbe9d0e5…`, `nosd` active, `llama-server` **sem
  `-ngl`** e uma só cópia; gate MQEMU `fails=0` mas `SKIP` no `nosd` (auto-login
  em ttyS0, lição 62). Residual: C5.1 parcial (nix-daemon/`trusted-users`),
  metal e lane S2/CUDA AWAITING_HW, retry do 400 provado só na função pura.
- **Colheita metal 2026-11-03 (OBSERVED, 1 boot)**: bundle do pendrive
  `NIXAIOSLOG` lido read-only (sem montar) e versionado em
  [`docs/evidence/hw/2026-11-03_2346Z/`](docs/evidence/hw/2026-11-03_2346Z/)
  — journal-boot/follow, jarbas-cage, firstboot, hotswap e harvest M3b.
  Provou: cage **active** em VT7, S2 carregou o 4B Q6_K (depois de um OOM no
  1.º arranque), mic **32044 B** sob `DeviceAllow=char-alsa`, transmute a
  recusar blobs (`blob_deliberate`). Derrotas registadas em
  `BOOT_TO_JARBAS.md`: `nvidia-smi` SIGSEGV mesmo com `nvidia-bin-elf-pf-w`,
  `voice.tts` ENOENT, STT `ok` com texto vazio, peers=0. Extração
  reprodutível: `scripts/extract-usb-logs.py` → `logs/` (gitignored).
  Memória: [`docs/memory/SESSION_20261105_USB_HARVEST_FIXES.md`](docs/memory/SESSION_20261105_USB_HARVEST_FIXES.md).
- **Tranche 0.7.0-alpha.6 (in-tree, HUD C1–C6 + pendrive FAT 1 GiB)**:
  correcções do HUD `orb_lab.html` dirigidas às fotos do metal
  0.7.0-alpha.5 — C1 rede por `link_state` real (`nos-observe::
  NetLinkState`), C2 badge de scope na energia, C4 painel `02e VOZ`
  separado + `PROPOR SUSPEND` completo + drawer de lições
  (`nix.open_lessons` com `hint_for()`), C5 prioridades 180° no topo,
  C6 coluna esquerda 25vw exacto. Código `2fa3e61`, artefactos 720p
  `01fd4f4`, docs `e60e930`/`9e3bfd3`. Rebuild `iso-hw-cuda`:
  ISO sha `5747c666…8a41` (`nlbyyyix40jb0i7c4gw90saj92ba4f6f`), `.img`
  8,75 GiB sha `09c4e76f…f194` com FAT32 **1 GiB** `NIXAIOSLOG`
  (`FAT_MB=1024`); S1 Astra + S2 4B + voz provados por squashfs (lição
  52). Evidência `docs/evidence/ISO_HW_CUDA_S1ASTRA_2026-11-04.md`;
  memória `docs/memory/SESSION_20261104_HUD_C1C6.md`. Metal AWAITING_HW
  (boot no pendrive novo não observado).
- **S1 swap 2026-11-04 (in-tree)**: a `iso-hw` arranca com o Qwen3.5-0.8B
  **editado pela Astra** (Weight-Space Programming, MIT CSAIL/RikiyaT —
  loop de thinking removido em down_proj L15/19, 0.016% dos pesos,
  sem treino). `qwen35-08b-q4km` → GGUF Q4_K_M NotCherry (541 903 776 B,
  LFS sha `0be511c8…`, `nix build` hash-verify PASS); fallback
  `qwen35-08b-q4km-base` (unsloth sem edito) também PASS. Commit
  `eba69c2`. Residual: edito sobreviver à Q4_K_M / `--reasoning off`,
  boot engine QEMU/metal OBSERVED pendentes. Evidência
  `docs/evidence/S1_ASTRA_EDIT_2026-11-04.md`. **ISO `iso-hw-cuda`
  construída 2026-11-04 (host OBSERVED): 8 323 596 288 B, sha
  `c2da2cfd…`; S1 Astra + S2 4B Q6_K no squashfs — ver
  `docs/evidence/ISO_HW_CUDA_S1ASTRA_2026-11-04.md`**; metal AWAITING_HW.
- **Tranche simbiose 2026-11-03 (in-tree)**: fábrica de apps **real** em Rust e
  Python (`nos-factory::scaffold`; RPC `factory.scaffold|factory.build`; CLI
  `factory-scaffold|factory-build`; cada app nasce com `flake.nix`), domínio
  Nix vivo (`nos-core::nix_domain`: factos da máquina + 20 lições do `AGENTS`;
  bloco `## Nix (live)` no prompt do córtex/hermes; rota `nix_keywords` S1/S2;
  RPC `nix.status|nix.ask`), HUD `02c SIMBIOSE` no ORB (`orb.symbiosis`).
  Evidência `docs/evidence/SYMBIOSIS_2026-11-03.md`; smoke
  `scripts/smoke-symbiosis.sh`. Not-tested: painel em QEMU/metal.

- **Closeout fórum 2026-11-02 (0.7.0-alpha.5 in-tree)**: F9/F10 harvest,
  N1/N2 memória, M1 telepathy, `M3b-audio` opt-in; commits `2d2fb01`/`da0100c`.
  Evidência `docs/evidence/FORUM_CLOSEOUT_0.7.0-alpha.5.md`. P0 seguinte =
  colheita metal Core7. `mesh-3vm-faults` / `iso-hw-cuda` AWAITING.
- **Tranche 0.7.0-alpha.4 (in-tree, frota-3 AIOS)**: `iso-hw-cuda` S1+S2,
  Landlock pós-spawn, mesh subnet, `brain.adapt` HITL, cage dbus/espeak, ORB
  2K/idle, `boots/` no NIXAIOSLOG, pack same-FS `mv`. ISO/img host OBSERVED
  (sha em `docs/evidence/METAL_FLEET3_0.7.0-alpha.4.md`). Metal **Core 7 240H**:
  HUD OBSERVED parcial (S2/CUDA/CAM; MIC OFF / sem som / peers=0); journal stick
  AWAITING (FAT Offline no Windows). i5/1050 + 7700G + mesh 3 AWAITING_HW.
  Ring continua desarmado. Memória: `docs/memory/SESSION_20261101_FLEET3_ALPHA4.md`.
- **Revisao de simbiose 2026-10-31 (in-tree)**: corrigidos 4 defeitos reais —
  `char-alsa` no DeviceAllow do nosd (voz: arecord/aplay como filhos sob
  DevicePolicy=closed), paridade `--fit/--reasoning` dos units do motor com o
  spawn do nos-cortex (`services.nosdEngine.fitCtx/reasoning`), `path` do
  unit `nosd-early-score`, e recall de memoria (acentos/pontuacao/hifen,
  stopwords PT/EN) + aviso de prompt-injection no bloco `## Memory recall`.
  Checks novos `nosd-voice-deviceallow` e `engine-fit-parity`. Mapa completo
  e roadmap em `docs/SYMBIOSIS_REVIEW_2026-10-31.md`. Sem metal/QEMU novo.
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
  StartLimit no UNIT nosd; `iso.nix` mantem 1777 na live — ver
  ARCHITECTURE/SECURITY, o 1777 e por-boot e intencional). G1: ancora do
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
- **Tranche 0.7.0-alpha.3 (in-tree, metal GPU + ISO 4B)**: fix ELF
  `nvidia-smi` (PF_W no PT_LOAD do `.dynamic`); Chromium cinematic (HOME/XDG,
  Crashpad, NNP off, Mesa iGPU); S1+S2 CUDA no `hw-cuda`; capacity
  igpu/dgpu + retry; `plan_route` no prompt do user; variante
  **`iso-hw-cuda-4b`** (só `Qwen3.5-4B-Q6_K`). ISO `67355245…` (7,2 GB),
  img `5bacea85…` (9,2 GB). Metal re-boot AWAITING_HW. Ver
  `docs/evidence/METAL_GPU_4B_0.7.0-alpha.3.md` e
  `docs/evidence/hw/2026-10-31_1944Z/`.
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
  `--disable-gpu`. ISO `2ee396fd…` (17,2 GB), img 21 516 779 520 B sha
  `4e339f37…` (tail-sha == ISO; `char-nvidia`/DeviceAllow, `char-alsa` +
  `XDG_RUNTIME_DIR` da revisão). **Metal AWAITING_HW** (drivers/GPU/voz).
  Ver `docs/evidence/hw/2026-10-31_0140Z/README.md`. Superado em parte por
  alpha.3 (ELF smi, cage HOME, S1 CUDA, ISO 4B).
- **M6 identidade**: HB Ed25519 assinado + pinagem TOFU + allowlist +
  anti-replay; `mesh-2vm-signed` PASS. Sem Noise (confidencialidade) e sem
  revogacao ainda.
- **M8 estado**: CRDT (LWW+VV) sobre o link assinado do M7 (`kind=2`);
  `crdt.put/get/sync`; `mesh-2vm-crdt` PASS. Anti-entropy = estado completo
  (sem delta/Merkle), em claro, sem persistencia nsgdb.
- **S1 skills trust**: BLAKE3 (64 hex) + manifesto `skill/<n>/meta` +
  `skill.explain <focus>` com score 0-100 e `why` no CRDT.
- **S2a publisher**: assinatura do manifesto (`name|focus|version|hash`) +
  allowlist `NOSD_SKILL_PUBLISHERS`; provenance `signed`. Feito.
- **S2b caps/versao**: caps declaradas validadas (allowlist `aios_ai.infer`,
  `log`), downgrade por `focus` recusado sem HITL. Em runtime a concessao e
  declaradas ∩ `NOSD_SKILL_GRANT_CAPS` (default `log`). Feito.
- **Skills v2 (in-tree, ABI 2)**: `nos-wasm` com `SandboxPolicy`
  (frugal/app), inspecao de imports (`aios_ai.infer`, `aios_log.write`),
  `result_ptr`/`result_len` (≤ 64 KiB, bounds-checked), fuel medido e cache
  BLAKE3 por processo (`FrugalRuntime::shared`). `nos-factory` promove com
  BLAKE3 + `ExecReport` e marca `placeholder` quando nao ha codegen.
  `skill.install`/`skill.push` partilham `install()` (hash → publisher → caps
  → downgrade → contrato → gravacao); `skill.run` devolve `fuel_used`,
  `output`, `result`/`result_b64` e `caps_declared/granted`. NixOS:
  `services.nosd.skillGrantCaps` / `skillAppProfile`; perfil `app`
  (1 MiB/500k) so com `NOSD_SKILL_APP_PROFILE=1`. Quota de execucao por
  peer (runs+fuel, janela deslizante, reserva atomica antes de ler o
  modulo, janela persistida em `<data-dir>/skill_quota.json` e assinada
  Ed25519 com a chave do no + ancora anti-apagamento/rollback) em
  `services.nosd.skillQuota*`; fuel medio entra no score do `skill.explain`
  (`why.fuel`). Sem identidade M6 a janela e declaradamente nao assinada
  (`signed=false`). **Falta**: assinatura do resultado, gossip de revogacao,
  quota distribuida. Ver `docs/SKILL_TRUST_AND_SELECTION.md` §3.1.
- **M9.1 SKILL_PUSH**: modulo viaja cifrado (`kind=3`) e so activa se o hash
  BLAKE3 bater; 512 KiB/push (base64 no envelope); push assinado e revogacao
  S2c; sem gossip de revogacao.
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
- **M9 skills**: `skill.install|list|run` + registo `skill/<n>/active` no
  CRDT; execucao frugal como em **Skills v2**. `skill.import` declarativo
  (`SKILL.md`) nao executa wasm. Quota de execucao por peer (Skills v2),
  local ao no.
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

## Evidência 0.7.0-alpha.3

`docs/evidence/METAL_GPU_4B_0.7.0-alpha.3.md`, `docs/evidence/hw/2026-10-31_1944Z/`.

## Evidência 0.7.0-alpha.2

`docs/evidence/ORB_UI_0.7.0-alpha.2.md`, `docs/evidence/artifacts/orb_ui_mesh_2nodes.jpeg`.

## Evidência 0.6.0-alpha.16

`docs/evidence/V9_DAG_KIOSK_0.6.0-alpha.16.md`, `docs/evidence/V8_OMH_ECO_0.6.0-alpha.15.md`, `docs/evidence/VAMPIRE_V0_V7_0.6.0-alpha.14.md`, `docs/evidence/QWEN35_0.8B_0.6.0-alpha.12.md`, `docs/RELEASE_0.6.0-alpha.11.md`, `docs/evidence/QEMU_0.6.0-alpha.11.md`, `docs/evidence/M5.1_3VM_FAULTS_0.6.0-alpha.10.md`, `docs/RELEASE_0.6.0-alpha.10.md`, `docs/evidence/QEMU_0.6.0-alpha.10.md`, `docs/evidence/M8.1D_NSGDB_BACKEND_0.6.0-alpha.9.md`, `docs/evidence/M8.1D_CRDT_ANCHOR_0.6.0-alpha.9.md`, `docs/evidence/S2C_SIGNED_PUSH_E2E_0.6.0-alpha.9.md`, `docs/evidence/S2C_REVOKE_0.6.0-alpha.9.md`, `docs/evidence/CLI_SKILL_AUTH_0.6.0-alpha.9.md`, `docs/RELEASE_0.6.0-alpha.9.md`, `docs/evidence/S2B_SKILL_CAPS_0.6.0-alpha.8.md`, `docs/evidence/S2A_SKILL_PUBLISHER_0.6.0-alpha.8.md`, `docs/evidence/S1_SKILL_TRUST_0.6.0-alpha.8.md`, `docs/evidence/M8.1e_CRDT_LOG_AUTH_0.6.0-alpha.8.md`, `docs/evidence/M8.1c_CRDT_COMPACT_0.6.0-alpha.8.md`, `docs/evidence/M8.1b_CRDT_LOG_0.6.0-alpha.8.md`, `docs/evidence/M8.1_DELTA_0.6.0-alpha.8.md`, `docs/evidence/M9.1_SKILL_PUSH_0.6.0-alpha.8.md`, `docs/evidence/M9_SKILLS_0.6.0-alpha.8.md`, `docs/evidence/M7_JOBS_0.6.0-alpha.8.md`, `docs/evidence/M6.1_NOISE_0.6.0-alpha.8.md`, `docs/evidence/M6_IDENTITY_0.6.0-alpha.8.md`, `docs/evidence/HITL_0.6.0-alpha.8.md`, `docs/RELEASE_0.6.0-alpha.7.md`, `docs/evidence/NIX_RING_0.6.0-alpha.7.md`, `docs/RELEASE_0.6.0-alpha.6.md`, `docs/evidence/SECURITY_S08_S09_0.6.0-alpha.6.md`, `docs/RELEASE_0.6.0-alpha.5.md`, `docs/evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md`, `docs/evidence/CAGE_SLINT_0.6.0-alpha.4.md`, `docs/evidence/VOICE_0.6.0-alpha.2.md`, `docs/evidence/VM_GGUF_0.6.0-alpha.2.md`, `docs/evidence/QEMU_0.6.0-alpha.2.md`.
