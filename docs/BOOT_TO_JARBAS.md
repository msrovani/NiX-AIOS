# Boot → JARBAS Jarvis-like — playbook NixAIOS

**Produto:** IDEA **#707** / ADR-0118 · workspace **0.6.0+** · volid ISO **`NIXAIOS051`**.  
**Aceite primário Jarvis-like:** **M3** (host/WSL + ISO live). **M4 metal** e **M5 frota** = continuidade honesty (nunca Ring0).  
**Doctrine:** JARBAS → Hermes → Cortex · CapGate server-authored · S1.2 sem seed GGUF/AgentJev na ISO · `autoSwitch=false` · Frugal wasmi path A · taxonomia ADR-0113.

**Não copiar** prosa/código de `neural-os-core`; só abstrair *como fazer* (IDEA/ADR por ID).  
**Colisão de nomes:** M0–M5 **deste** playbook ≠ ondas M\* ADR-0106 ≠ O0–O7 BEI/0092.

Gate objectivo (stdout = ground truth da IA):

```bash
bash scripts/gate-boot-to-jarbas.sh          # host TCP default
bash scripts/gate-boot-to-jarbas.sh --stage M0
NOSD_TCP=127.0.0.1:7700 bash scripts/gate-boot-to-jarbas.sh
```

---

## Distinção dos rings

### Eixo A — privilege

| Ring | Neste produto |
|------|----------------|
| **Ring0** bare | **OUT_OF_SCOPE** — não reimplementar no flake |
| **Ring3** bare | só espelho de doutrina |
| **Ring3_Linux** | `nosd` + `nos-jarbas` sob systemd + CapGate + Landlock (**anel real**) |

Metal stick M4 = **Ring3_Linux em hardware** — **não** Ring0.

### Eixo B — portabilidade (ADR-0118)

| Camada | Papel |
|--------|--------|
| **PORT** | CapGate, Hermes OPAR, Factory Frugal, ModelHub+`score_v0`, BEI, nos-memory, mesh wire |
| **ADAPT** | cage/Slint, llama/GGUF, HF fetch, voz Whisper/Piper, capacity sysfs/`nvidia-smi` |
| **DELEGATE_LINUX** | NIC, NVMe, DRM, NM, seatd, PipeWire, systemd |
| **BAREMETAL_ONLY** | Limine/IDT/MMIO/CFS bare — **nunca** OK aqui |

### AI_GATE por ring

1. OK deve citar `ring=` + `layer=` + `claim_status=` (OBSERVED\|VERIFIED\|IMPLEMENTED\|HYPOTHESIS\|UNKNOWN\|BLOCKED\|AWAITING_HW).
2. Não promover PORT-contrato a VERIFIED se só stub ADAPT (ex.: Trinity BitNet).
3. Falha DELEGATE (sem DRM/NM) ≠ falha do agente PORT.
4. Sucesso Ring3_Linux **nunca** fecha residual Ring0.

---

## IDEA_BANK — filtro (#707)

| IDEA | Uso |
|------|-----|
| **#707** | Canónico deste playbook |
| **#706** BitFuzzy | **REJECT** hub/boot Nix |
| #512, #535, #468, #311/#0078, #469, #470–478, #314f, #310/#315, #315.25, #588/#0106, #189/#0081, #336, #539, #661/#665, #199, #208 | *Como fazer* PORT/ADAPT nas etapas abaixo |
| #424–432, BAR/W2A8 device, WiFi FW bare, voz sherpa supersedida, JS kernel | **REJECT** |

---

## Rubrica universal

| Campo | Função |
|-------|--------|
| GOAL | Intenção |
| SEEK | Paths / RPC / slog |
| MEASURE | Comando + predicado |
| OK / LOST | Vitória / derrota |
| AI_GATE | OK→avançar; LOST recuperável→1 retry; LOST policy→parar+ledger |

Anti-alucinação: UI completed ≠ aceite; `cargo test` ≠ metal; `cpu-echo` ≠ MoE; `EngineAbsent` = OK honesty, não OK capability.

**Âncoras:** `nosctl health|score|observe|chat|route`; `journalctl -u nosd -u jarbas-cage -u nosd-early-score -b`; `ls /var/lib/nosd/{boot-score.json,firstboot.*,models,nsgdb}`.

---

## M0 — Substrato vivo

**ring=Ring3_Linux** · **layer=PORT+ADAPT+DELEGATE**

| | |
|--|--|
| GOAL | Daemon notify, CapGate, Landlock, memória, score honest |
| SEEK | `nosd` active; sock/TCP; slog landlock; recall |
| OK | `health.status=ok`; `memory_ok`; landlock SoftAllowlist\|Verified; CapGate ignora bitmask cliente |
| LOST | unit failed; EACCES nsgdb; CapGate confia no cliente |
| refs | #512, #208, #539, ADR-0088/0092/0113 |

## M1 — Early + capacity + BOOT SCORE

**ring=Ring3_Linux** · **layer=ADAPT** sobre DELEGATE · S1 keyword PORT thin

| | |
|--|--|
| GOAL | HW observado pré-getty; ngl sugerido **sem** GGUF (S1.2) |
| SEEK | `nosd-early-score`; `observe-early.json`; `boot-score.json`; `NOSD_LLAMA_NGL`; `s1_backend` |
| OK | JSONs parseáveis; posture honesta; S1 keyword (não AgentJev no sysinit) |
| LOST | JSON ausente; VRAM fake; AgentJev no ISO = **REJECT** |
| refs | #468, #535, #539, ADR-0092 |

## M2 — Firstboot humano

**ring=Ring3_Linux** · Nix glue + DELEGATE

| | |
|--|--|
| GOAL | Rede/idioma/teclado/fuso; persist; neural quieto |
| SEEK | `firstboot.step|done|partial`; `network.conf`; `locale.conf`; `host-configuration.nix` |
| OK | Q1–Q4 gravados; Q5 transmute `--confirm` (download fail tolerado se cage sobe) |
| LOST | EACCES silent; abort sem partial; `gui_failed` permanente |
| refs | #707 firstboot path |

## M3a — Transmute + modelos (Trinity honesty)

**ring=Ring3_Linux** · **layer=PORT+ADAPT** · Trinity = hub+`score_v0` (**≠** ROUTER.BITNET)

| | |
|--|--|
| GOAL | capacity→plan→HITL `blob_deliberate`→hub→hot_load |
| SEEK | `model.plan` / `model.list`; errors transmute; engine posture |
| OK | plan coerente; fetch CapGate; hub≥1 GGUF **ou** cpu-echo doc; S1 AgentJev só pós-HITL LOO |
| LOST | download sem HITL; path fantasma; fingir CUDA |
| refs | #535, #468, #311, #588, ADR-0078/0106 |

## M3b — Hotswap → cage → JARBAS

**ring=Ring3_Linux** · **layer=ADAPT** (+ DELEGATE seatd)

| | |
|--|--|
| GOAL | Slint tty7; chat Hermes; orb `:7701` |
| SEEK | `jarbas-cage` active; `hotswap.log`; `firstboot.done`; `nosd` health (socket) |
| OK | cage up; chat round-trip; sem X11 |
| LOST | `.failed`; DRM vazio |
| refs | #310/#315, #0090, ADR-0036/0058 |

## M3c — MoE Continuous + Remember

**layer=PORT** (+ ADAPT S2 llama)

| OK | `hermes_ticks`↑; S2 drena ou pending honesto; recall após chat |
| LOST | ticks stuck; EACCES write |
| refs | #199, #315.20, #535 |

## M3d — Factory wasmi + Seek

**layer=PORT**

| OK | `smoke-frugal-fleet.sh` PASS; seek NetAbsent\|candidates; spawn CapGate |
| LOST | synth sem seek; wasmtime path A |
| refs | #469, #661/#665 (`NOSD_AUTOLEARN` default off) |

## M3e — BEI + Sleep thin

**layer=PORT scaffold**

| OK | ≥1 consolidate `session/sleep`; sem panic |
| LOST | claim SleepCycleAgent bare 1:1 |
| refs | #470–478, #314f |

## M3f — STT → Hermes → TTS

**layer=ADAPT→DELEGATE**

Estágios honesty: `mic → vad → wake → stt → handle → tts → out` (PipeWire/CLIs).

| OK | `voice.status` Ready **ou** EngineAbsent; `voice.turn` se Ready |
| LOST | crash; silent fail |
| refs | #315.25, ADR-0045 |

**Fecho M3 Jarvis-like:** M3b + (GGUF chat **ou** cpu-echo doc) + (voice Ready **ou** Absent honesty) + M3c remember.

## M3g — nsgdb facade

| OK | `seg.bin` cresce; recall lexical; L5–L7 residual no ledger |
| LOST | VERIFIED MCP sem MCP |
| refs | #0091 |

## M4 — Metal stick (`Ring3_Linux/metal`)

| OK | journal NIXAIOS051; pós-Q5 sem cascata EACCES; cage; evidência ledger |
| LOST | emergency midia; só preflight = AWAITING_HW; claim Ring0 |
| Campanha | [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) · `scripts/metal-iso-deep-preflight.sh` |

## M5 — Frota (0081 A/B; C residual)

| OK | `mesh_peers≥1`; SoulMirror tick; offload **hint** (remote infer = residual) |
| LOST | claim remote S2 infer sem wire |
| Campanha | [`FLEET_LAB.md`](FLEET_LAB.md) |

---

## Cheatsheet units / env

**Units:** `nosd` · `nosd-early-score` · `nosd-hotswap-aios` · `jarbas-cage` · `jarbas-hub` · `seatd`

**Env:** `NOSD_SOCKET` · `NOSD_DATA_DIR` · `NOSD_S1_BACKEND` · `NOSD_S1_BLOB` · `NOSD_LANDLOCK` · `NOSD_LLAMA_CLI` · `NOSD_LLAMA_NGL` · `NOSD_HERMES_TICK_SECS` · `NOSD_MESH_BIND` · `NOSD_ORB_LAB` · `NOSD_AUTOLEARN` · `NOSD_MEMORY_BACKEND` · `NOSD_LOCALE`

---

## Lab QEMU (≠ metal)

```bash
# WSL — ISO remigração (ou NIXAIOS051 quando rebuild)
ISO=/mnt/c/Users/msrov/Downloads/nixaios-minimal-s529-remig.iso \
  LOG=/tmp/nixaios-qemu.log DISPLAY_MODE=serial \
  bash scripts/run-iso-qemu-wsl.sh
# Gate honesty pós-boot serial:
NIXAIOS_QEMU_LOG=/tmp/nixaios-qemu.log bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

**claim:** `Ring3_Linux/qemu` OBSERVED ≠ `Ring3_Linux/metal` VERIFIED.

## Residuais pós-playbook (honestidade)

| Gap | Status | Próximo |
|-----|--------|---------|
| M3b cage + chat e2e | AWAITING_HW / live | journal cage+`nosctl chat` |
| HITL cards | **IMPLEMENTED** thin `ui.card` + `card:` prompt; Slint rich residual | widgets Slint |
| AgentJev LOO load | honesty ScorerAbsent + blob_deliberate; load runtime residual | `NOSD_S1_BLOB` pós-HITL |
| M4 metal journal | AWAITING_HW | [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) |
| M5 0081 A/B peers | AWAITING_HW | [`FLEET_LAB.md`](FLEET_LAB.md) |
| Trinity neural | REJECT bare / score_v0 OK | não BitNet hub |

## Non-goals

Seed ISO S1 · X11 · wasmtime path A · Ring0/BAREMETAL_ONLY · BitFuzzy hub · copiar docs bare.
