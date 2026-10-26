# NixAIOS — Migration Matrix (PORT lines first)

Taxonomia: `PORT` | `ADAPT` | `DELEGATE_LINUX` | `BAREMETAL_ONLY` | `SUPERSEDE_NIX` | `RESEARCH` | `DEAD_WEIGHT`

Produto paralelo em `nixaios/` — bare-metal permanece canônico. Evidência: ADR-0113.  
**Camadas (visão):** [`adr-addenda/0118-portability-layers.md`](adr-addenda/0118-portability-layers.md) — (1) cognitivo PORT · (2) UI/áudio ADAPT · (3) drivers BAREMETAL_ONLY/DELEGATE.

### Resumo das 3 camadas

| Camada | O que migra | Substrato NixOS | Honesty MVP |
|--------|-------------|-----------------|-------------|
| **1 PORT** | Agentes, CapGate, wasmi, mesh, nsgdb, route/HotSwap | daemon `nosd` + crates `std` | BitNet/Falcon3/SleepCycle pleno ≠ ainda no nosd |
| **2 ADAPT** | JARBAS, áudio, FS cognitivo | Wayland+cage, PipeWire (F7), dirs FHS | X11 proibido; FUSE NeuralFS RESEARCH |
| **3 DELEGATE** | Boot, IDT, NIC/NVMe/xHCI, PMM, Ring3 bare | kernel Linux + systemd | não reimplementar no flake |

## Eixos viáveis (prioridade)

| ID | Nome | Disposicao | Alvo NixAIOS | Fase | Evidencia |
|----|------|------------|--------------|------|-----------|
| A-0059 | App Factory path A | PORT | `nos-factory` | F3 | wasmi self-test + CapGate |
| A-0081 | Mesh P2P | PORT | `nos-mesh` | F5 | TOFU/FRAG/SkillSync frames |
| A-0057 | Compute dispatch | PORT/ADAPT | `nos-cortex` dispatch | F2 | fail-closed local |
| T-mesh_distrib | Distributed assign | PORT | `nos-cortex::distrib` | F5 | capacity_weighted_assign |
| A-0060 | BEI | PORT | `nos-bei` (tick fino) | pós-MVP | budget/affect hooks |
| A-0078 | ModelHub multi-slot | PORT/ADAPT | `nos-cortex` hub | F2 | validate_for_slot + LOO |
| A-0091 | neural-sgdb | PORT | `nos-memory` | F4 | FileStorage remember/recall |
| A-0088 | Premissa AIOS | PORT | doutrina nosd | F0 | ADR-0118 |
| A-0113 | Missão sistêmica | PORT | honesty nosd | F0 | NOSD_SCORE |

## Tecnologias (amostra PORT/ADAPT)

| ID | Nome | Disposicao | Alvo | Fase |
|----|------|------------|------|------|
| T-1.1 | Agent ontológico | PORT | `nos-hermes` | F5 |
| T-1.6 | Trinity MoE | PORT | `route`+`router_score`+`S2Queue` | F2 | score_v0+keyword host VERIFIED (≠ BitNet); S2 async VERIFIED |
| T-2.9 | EventBus | PORT | `nos-bus` | F1 |
| T-2.10a | CapGate WASM | PORT | `nos-wasm` + `agent.spawn` | F1/F3 | Factory A + wasm_b64 host VERIFIED; path B/C Absent |
| T-2.10f2 | AirLLM/GGUF | ADAPT | HeaderScanner + HotSwap + `slot_from_path` | F2 | multi-slot scan host VERIFIED |
| T-1.7 | UI Orb/HUD | ADAPT | `nos-jarbas` Slint+slog + cage | F6 | host+flake recipe VERIFIED (s505); nix ISO/DRM metal UNKNOWN |
| T-4.18+ | Mesh lab | PORT | `nos-mesh` | F5 |
| T-7.x | BitNet/ModelHub | PORT/ADAPT | hub + llama.cpp | F2 |
| T-2.1 | Limine | BAREMETAL_ONLY | — | — |
| T-2.2 | IDT | BAREMETAL_ONLY | — | — |
| T-2.3 | PMM | DELEGATE_LINUX | kernel | — |
| T-2.6 | SMP trampoline | BAREMETAL_ONLY | — | — |
| T-2.8 | CFS próprio | DELEGATE_LINUX | CFS Linux | — |
| T-2.10g | Ring3 bare | SUPERSEDE_NIX | namespaces/seccomp | defer |
| T-3.x | BAR Mapped físico | ADAPT | CUDA/HIP Mapped honesty | F2+ |
| T-5.x | HDA bare | DELEGATE_LINUX | PipeWire | F7 |
| T-6.x | ATA/FAT drivers | DELEGATE_LINUX | ext4/btrfs | — |

## ADRs (amostra)

| ID | Disposicao | Nota NixAIOS |
|----|------------|--------------|
| 0001–0005, 0009 | BAREMETAL_ONLY / DELEGATE | boot/IRQ/paging |
| 0041 CapGate P3 | PORT | in-process + Landlock |
| 0052 Artifact | PORT | promote assinado |
| 0056 Device LEGO | ADAPT | udev/nixos hardware |
| 0058 Cards UI | ADAPT | Slint widgets |
| 0077/0102 Ring3 | SUPERSEDE_NIX | cgroup/seccomp |
| 0079→0086 Install | SUPERSEDE_NIX | ISO + nixos-rebuild |
| 0086 OTA | ADAPT | geracoes Nix |
| 0089 malha whitepaper | RESEARCH | Fase E host apps ≈ este produto |
| 0089 smp-runqueue | BAREMETAL_ONLY | conflito ID — nao fundir |
| 0090 Jarbas v2 | ADAPT | nos-jarbas |
| 0092 Boot obs | ADAPT | NOSD_SCORE / journald |
| 0101 Falcon3 lab | PORT contratos | hub slots |
| 0112 BAR | ADAPT | CUDA malloc honesty |
| 0117 BitFuzzy | DEFER hub | host-only |

## Onda remigração + release 0.5.1 — ledger (fonte de verdade)

Plano Cursor `nixaios_remigração_34e29087` → status item-a-item; ship [`RELEASE_0.5.1.md`](RELEASE_0.5.1.md) (volid `NIXAIOS051`, persist Linux-first; base [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md)).

**[`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md)** + **[`REMIGRATION_PLAN_ACCEPTANCE.md`](REMIGRATION_PLAN_ACCEPTANCE.md)**.

| Marco | Status | Nota |
|-------|--------|------|
| CapGate server-authored (I.1) | **host VERIFIED** | `CapRegistry`; client ALL ignored |
| SITE_POLICY + ISO autoSwitch=false (NX.0) | **host VERIFIED** | NIXAIOS529 + policy.matrix |
| NX.1–NX.6 Tema NX (ecacc3de) | **host VERIFIED** / PARTIAL NX.5 | ver `REMIGRATION_NX_ACCEPTANCE.md` |
| Seek-Before-Generate (F.3) | **PARTIAL** | GH/GL/crates HTTP + allowlist |
| Landlock syscall (Linux) / SkillOpt / AirLLM plan | **PARTIAL** | ver ledger — metal AWAITING |
| nsgdb default seg.bin + TOFU/FRACK UDP + CapGate I.1 | **host VERIFIED** | plan c2d76600 aceite; lab/metal residual |
| S1.2 seed ISO | **REJECT** | runtime `blob_deliberate` |
| Anti-padrão | — | UI todo completed ≠ aceite plano |

## Onda 8 — Mesh FRAG + Artifact MD + SleepCycle thin (s509)

| Item | Disposition | Evidência | Status |
|------|-------------|-----------|--------|
| FRAG\0 reassembly CAP8 | PORT 0081 | `nos-mesh::frag` + UDP `on_datagram` | host VERIFIED (unit) |
| FRACK/ACK seletivo | PORT residual | — | AWAITING |
| Artifact MD deny-by-default | PORT 0052 | `validate_artifact_md` + IPC | host VERIFIED (unit; Ed25519 residual) |
| SleepCycle 5 fases | PORT thin 1.4 | `nos-bei` SleepPhase no tick | IMPLEMENTED (≠ bare SleepCycleAgent) |
| Smoke ISO QEMU suggest | prova | Downloads s507 ISO | AWAITING |
| Metal Rufus | HITL | — | AWAITING_HW |

## NX remigration (s529 / NX.0)

| Item | Disposition | Evidência | Status |
|------|-------------|-----------|--------|
| CapGate NIX server-authored | PORT 0041 | `nosd-nix-apply` root + HITL env | **fazendo** |
| Seek-Before-Generate (F.3) | PORT | `nos-factory` seek + plan IPC | **fazendo** |
| Nix ring rollback oneshot | ADAPT 0086 | `nix-rollback.request` + generation file | IMPLEMENTED (module) |
| nosd-engine@ dGPU | ADAPT F2 | template llama-server ≠ cage iGPU | IMPLEMENTED (module) |

Ondas futuras preenchem as ~100 linhas restantes de TECNOLOGIAS §1–15 e INDEX completo.
