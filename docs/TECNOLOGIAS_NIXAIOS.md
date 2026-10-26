# TECNOLOGIAS — NixAIOS (espelho PORT/ADAPT)

**Release 0.5.1** — volid `NIXAIOS051`; persistência Linux-first (`/var/lib/nosd`); FAT só `NIXAIOS_USE_FAT_PERSIST=1`. Ver [`RELEASE_0.5.1.md`](RELEASE_0.5.1.md) / [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md).

Catálogo vivo do produto paralelo. Bare-metal continua em [`TECNOLOGIAS.md`](https://github.com/msrovani/neural-os-core/blob/main/TECNOLOGIAS.md) §1–15.

**Canónico WASM:** ADR-0059 (Caminho A wasmi). **ADR-0032 = superseded** — não usar como inventário.
**Frugal NixAIOS (0118):** heap **256 KiB = 4 páginas** WASM (página=64 KiB). Fuel **100_000**.
Bare-metal `hermes::wasmi_rt` usa `DEFAULT_FUEL=5_000_000` — perfil diferente de propósito.

Taxonomia ADR-0113: scaffold **IMPLEMENTED** ≠ forward GGUF/CUDA/metal **VERIFIED**.  
Camadas bare→NixOS: [`adr-addenda/0118-portability-layers.md`](adr-addenda/0118-portability-layers.md) + [`MIGRATION_MATRIX.md`](MIGRATION_MATRIX.md).  
**Plano remigração 55 itens (s529):** [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md) — DONE/PARTIAL/NOT_DONE (não usar checklist Cursor como aceite).

| # | Tecnologia | Disposicao | Crate / path | Status (honest) |
|---|------------|------------|--------------|-----------------|
| N.1 | nosd daemon (Tokio + notify) | PORT wire | `nosd` | IMPLEMENTED; smoke host OBSERVED |
| N.2 | EventBus std | PORT | `nos-bus` | IMPLEMENTED scaffold |
| N.3 | CapGate IPC server-authored | PORT | `CapRegistry` + `agent` | **I.1** host VERIFIED (client ALL ignored) |
| N.3b | SITE_POLICY Auto/Confirm/Escalate/Deny | NOVO | `nos-core::policy` + `policy.matrix` | **NX.0** host VERIFIED; ISO autoSwitch=false |
| N.3h | Observe journal/udev/NM→EventBus | NOVO | `nos-observe` | **NX.3** host VERIFIED; DBus residual |
| N.3i | ai-managed schema + build-vm | NOVO | `nix_managed` + cortex-nix-ring | **NX.1** host VERIFIED |
| N.3j | Actuators PipeWire/GPU/recipe | NOVO | `actuators` RPC | **NX.4** host VERIFIED |
| N.3c | Runtime blob/firmware deliberado (sem seed ISO) | NOVO | `nos-hermes::blob_deliberate` | **S1.2 REJECT**; fetch pós Cortex→Hermes→JARBAS HITL |
| N.3d | nsgdb seg.bin L2/L3 (não JSONL default) | NOVO | `nos-memory::seg` | **DB.1** host VERIFIED default; MCP CRDT AWAITING |
| N.3p | Persist Linux-first pós-Q5 (0.5.0) | NOVO | `transmute::resolve_models_dir` + Landlock `/mnt/nixaios-log` | **host VERIFIED** testes; metal EACCES mitigado; boot metal AWAITING_HW |
| N.3e | PackageStore FNV+Ed25519+FederatedIndex | NOVO | `nos-factory::store` | **P.12** host VERIFIED; trusted key ring fleet residual |
| N.3f | Mesh TOFU settle + FRACK UDP | NOVO | `nos-mesh` + `udp` | **M.1** host VERIFIED; lab 2-nó AWAITING |
| N.3c | Seek-Before-Generate + store federado | NOVO | `nos-factory::seek/store` + Hermes | **F.3/P.12** mesh→GH/GL/crates/MCP; FederatedIndex; NetAbsent; SESSION_529 |
| N.3k | Host perf + host-configuration firstboot | ADAPT | `aios-host-perf` + transmute | swappiness/governor/OOM; fragmento Nix sem switch live; triagem prompt Minimal |
| N.3l | nosd-early observe+BOOT SCORE pré-getty | NOVO / B.1 | `nosd-early.nix` + `nosctl observe` | keyword S1; AgentJev ISO REJECT; ProtectSystem=strict no cage REJECT (DRM); engine/cage OOM split; Master Unification triage |
| N.3m | Master Unification W1–W6 host | NOVO | score+S1 gate+Landlock env+cage After+guard ISO | host VERIFIED (SESSION_529); metal BOOT_SCORE AWAITING |
| N.3d | nos-observe + presence + actuators | NOVO | `nos-observe` + `actuators` | **NX.3/4** sysfs best-effort; Windows empty OK |
| N.3e | Landlock soft + nosd-engine@ dGPU | ADAPT | `landlock.rs` + `nosd-engine.nix` | SoftAllowlist; kernel Landlock = AWAITING |
| N.3f | LatentBus + Decision + Agency + Membrane | PORT | `nos-bus::latent` + decision + agency | **P.2/S1/A.1/P.6** IMPLEMENTED scaffold |
| N.3g | FRACK + FLEET_HEALTH + AutoLearn | PORT | `nos-mesh` + `autolearn` | **M.1/P.13/T.2** host tests |
| N.3 | CapGate IPC (legacy note) | PORT | `nos-ipc` | superseded by N.3 server-authored |
| N.4 | ModelHub 8 slots + LOO + `slot_from_path` | PORT | `nos-cortex` | multi-GGUF scan host VERIFIED (s503) |
| N.5 | HeaderScanner GGUF/Safetensors | PORT | `nos-cortex` | IMPLEMENTED header only |
| N.6 | HotSwapController + S2Queue | ADAPT | `nos-cortex/hotswap` + `s2_queue` | unload + `hot_load_llama` + S2 host VERIFIED (s504–507); Llama CLI opt-in; EXL2/vLLM **EngineAbsent** |
| N.6b | HF/HTTPS GGUF fetch + suggest | ADAPT | `nos-cortex::fetch` + IPC `model.*` | ureq TLS + CapGate NET\|FS (s507); tempo troca 1–4s = HYPOTHESIS; lab tok/s UNKNOWN |
| N.7 | App Factory path A Frugal + Seek gate | PORT | `nos-factory` + `spawn_wasm_agent` | Frugal + Seek-before-synth host VERIFIED (s529); path B/C Absent |
| N.8 | wasmi Frugal 256KiB/100k | PORT política | `nos-wasm` | Host **OBSERVED**: fleet 32+128 identity + fuel trap (`smoke-frugal-fleet.sh`); fuel dinâmico = HYPOTHESIS |
| N.9 | neural-sgdb memory | PORT | `nos-memory` | IMPLEMENTED facade file; recall WSL **OBSERVED** |
| N.10 | Hermes agentic S1/S2 | PORT | `nos-hermes` | OPAR+S2Queue+score_v0 host VERIFIED; GGUF remember WSL OBSERVED |
| N.10b | Router score_v0 | ADAPT | `nos-cortex::router_score` | pesos fixos + margem; ≠ ROUTER.BITNET (s505) |
| N.11 | BEI tick + SleepCycle thin | PORT | `nos-bei` | budget+affect+5 fases no tick (s509); ≠ SleepCycleAgent bare |
| N.12 | Mesh ADR-0081 HB+FRAG+FRACK | PORT | `nos-mesh` | HB+TOFU+FRAG+FRACK TX UDP (s529); lab 2-nó residual |
| N.12b | Artifact MD 0052 | PORT | `nos-factory::validate_artifact_md` | deny-by-default frontmatter+seções; Ed25519 residual |
| N.13 | Distrib assign | PORT | `nos-cortex::distrib` | IMPLEMENTED scaffold |
| N.14 | JARBAS persona + Slint/cage | ADAPT | `nos-jarbas` + `cage` | doutrina: UI=JARBAS→Hermes→Cortex; janelas HITL; HUD thin s511; Wi‑Fi card AWAITING |
| N.14c | HITL window connectors | ADAPT 0058 | `ui_window`+WindowBroker+IPC | open/list/action VERIFIED host; NM Act AWAITING |
| N.14d | Orb cinematic lab | ADAPT VoiceOrbs+iron | `orb`+OrbBroker+HTTP lab | sense cam/mic VERIFIED host; Vision MoE AWAITING |
| N.14b | Smithay compositor | RESEARCH | — | futuro; MVP = `cage` |
| N.16b | Flake Slint ISO | NOVO | `flake.nix` cargoExtraArgs | recipe ON; ISO s507 Downloads VERIFIED; rebuild s509 AWAITING |
| N.15 | Voice Whisper/Supertonic | ADAPT | `nos-voice` | EngineAbsent stub |
| N.16 | NixOS flake + modules | NOVO | `nixaios/` | IMPLEMENTED scaffold |
| N.17 | ISO offline | NOVO | `images/iso.nix` | QEMU OBSERVED; metal UNKNOWN |
| N.18 | ai-managed.nix + anel Cortex | NOVO | `nix.propose`/`nix.apply` + `nosd-nix-apply` root | AUTO_SWITCH arma apply; switch no helper root (s517) |
| N.18b | model.plan HF | ADAPT | `nos-cortex::plan_models_for_capacity` | capacity→quant→HF search (s517) |
| N.18c | Wi‑Fi HITL NM | ADAPT | `wifi_nm` + `nmcli` + ui hitl_wifi | scan+connect CapGate HITL (s517) |
| N.19 | CUDA GGUF `-ngl` | ADAPT | `capacity` nvidia-smi + `NOSD_LLAMA_NGL` + flake `llama-cpp-cuda` | Host GPU WSL **OBSERVED** (RTX 3050); nixpkgs `llama-cpp` = **CUDA_ENGINE_ABSENT**; forward GPU = AWAITING `.#llama-cpp-cuda` |
| N.20 | Metal USB ISO | NOVO | `metal-iso-deep-preflight.sh` + `metal-usb-handoff.ps1` + Rufus DD | Preflight software **OK**; boot metal **AWAITING_HW** |

### Onda 2 — fecho software (2026-10-21)

| Item | Evidência | Status |
|------|-----------|--------|
| Honesty docs 0032/fuel/páginas | TECNOLOGIAS + ADR refs | DONE |
| HotSwap GGUF `llama-completion` | smoke-cognitive-loop | OBSERVED WSL |
| Capacity VRAM | `posture=nvidia_smi` slog | OBSERVED WSL |
| Cognitive loop | tokens + remember/recall | OBSERVED WSL |
| wasmi Frugal fleet | smoke-frugal-fleet 32+128 | OBSERVED host |
| nix.propose HITL | CapGate NIX_WRITE staging | IMPLEMENTED |
| CUDA `-ngl` | smoke-cuda-honesty → ENGINE_ABSENT | OBSERVED honesty |
| Metal USB boot | Rufus DD + copytoram | **AWAITING_HW** |
