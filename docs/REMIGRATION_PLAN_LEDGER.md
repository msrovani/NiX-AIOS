# NIXAIOS Remigração — Ledger do plano (fonte de verdade)

**Plano:** Cursor plan `nixaios_remigração_34e29087` (aprovado 2026-10-25).  
**Sessão audit:** SESSION_529.  
**Regra:** UI “todo completed” ≠ aceite do plano. Taxonomia ADR-0113:

| Status ledger | Significado |
|---------------|-------------|
| **DONE** | Código + teste host (ou Nix wired) cobre o *goal* do item |
| **PARTIAL** | Existe scaffold / wire parcial; goal do plano **não** fechado |
| **NOT_DONE** | Overclaim ou ausente — reabrir na próxima onda |
| **PREEXIST** | Já existia antes desta remigração; só referenciado |

Contagens (fecho plano c2d76600 host): **DONE ~18 · PARTIAL ~35 · NOT_DONE ~2 · REJECT 1**.  
**0.5.0/0.5.1 (2026-10-25):** P0 metal pós-Q5 Linux-first + Landlock FAT allow + CRDT/SoulMirror thin + llama_server_posture + SelfHeal deny stub + docs modernas — ship Git [`RELEASE_0.5.1.md`](RELEASE_0.5.1.md). Ondas = host-OK; metal/lab e2e = residual AWAITING_HW (sem overclaim).

**0.5.2 (2026-10-25):** Revisão estrutural A+B+C — archive s5xx, IPC registry, autolearn off, hub único, self.heal RPC, HermesEngine::boot, rpc/ fatiado, firstboot/hotswap externos — [`RELEASE_0.5.2.md`](RELEASE_0.5.2.md). Volid permanece `NIXAIOS051`.

**0.5.4 Trust & Truth (em curso, 2026-10-26):** S-01/S-02/S-03 — identidade IPC por `SO_PEERCRED`/token, `agent`/`caps` legado ignorados, `security.rotate` com revogação; HTTP lab `/rpc` com token/same-origin/limites. Evidência: `nosd/tests/ipc_auth.rs`, `cargo test --workspace`, `clippy -D warnings`. Plano: [`IMPROVEMENT_PLAN.md`](IMPROVEMENT_PLAN.md). Ring Nix continua desarmado até S-04.

**Boot→JARBAS (playbook M0–M5, 2026-10-26):** [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md) + gate + [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) + [`FLEET_LAB.md`](FLEET_LAB.md) + [`memory/SESSION_BOOT_JARBAS.md`](memory/SESSION_BOOT_JARBAS.md). M3: `route.explain` trinity/`Decision`/S1 LOO + voice stages + **`ui.card` HITL thin**. Gate `MQEMU` = QEMU OBSERVED ≠ metal. **M4/M5** = **AWAITING_HW**.

---

## Tema 0 — Isolamento

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| I.1 | CapGate server-authored; deny client ALL | **DONE** (0.5.4) | `PeerProfile::from_unix_cred`; token-first; `agent`/`caps` ignorados; `security.rotate`; `ipc_auth.rs` (TCP + Unix) |
| I.2 | Host-imports wasmi CapGate + Frugal prove deny | **PARTIAL** | Frugal + `aios_ai::infer` CapGate; fuel trap pré-existente; prove deny INFER OK |
| I.3 | systemd Protect* + Landlock **real** NIX_WRITE | **PARTIAL**↑ | 0.5.0: allowlist +`/mnt/nixaios-log`; `path_allowed` teste FAT; Verified só Linux+ABI; lab metal AWAITING |
| I.4 | Factory B/C PathBcAbsent | **DONE** / PREEXIST | `FactoryError::PathBcAbsent` |
| I.5 | Engine dGPU cgroup ≠ cage iGPU | **PARTIAL**↑ | engine OOM=+200 MemoryMax DevicePolicy=closed (sem DRM); cage DRM+OOM=-500; `lab-oom-engine.sh`; e2e metal AWAITING |

## Tema NX — NixOS Neural Ring

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| NX.0 | Matriz Auto/Confirm/Escalate/Deny; ISO autoSwitch=false | **DONE** | `policy_matrix_prod` + `policy.matrix` RPC; ISO autoSwitch=false |
| NX.1 | Schema ai-managed + build-vm + HitlConfirm + generation/rollback | **DONE** (host) | schema deny-list; `nix.build_vm`; HitlConfirm; buildVmFirst=true default; metal e2e residual |
| NX.3 | nos-observe journald+udev+NM DBus+hwmon→EventBus | **DONE** (host) | journalctl/nmcli/sysfs + `OBSERVE_SNAPSHOT` bus; DBus linkado **NOT** |
| NX.4 | Actuators CapGate NM/PipeWire/GPU/systemctl/DeviceRecipe | **DONE** (host) | systemctl+audio.volume+gpu.plan+device.recipe; NM wifi pré-existente |
| NX.5 | Capacity contínuo; presence; ngl/ROCm/OpenVINO; cage≠engine | **PARTIAL** | Continuous ngl + `accel` cuda/rocm/openvino path-detect; runtime ROCm/OV **NOT** |
| NX.6 | CognitiveAct OPAR universal + seek audit | **DONE** (host) | `seek_audit` em CognitiveAct; factory.seek/autolearn/spawn OPAR; SelfHeal residual |
| NX.7 | PackageHub store federado | **PARTIAL** | `PackageStore` + seek; Ed25519/sandbox militar pleno **NOT** |

## Tema M — Mesh Brain

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| M.1 | FRACK, TOFU settle, CapacityScore, roles, election | **DONE** (host) | `frack`+UDP TX pós-FRAG; `tofu_settled` 130 ticks; election CapacityScore; testes 21/21; lab 2-nó e2e = residual |
| M.2 | nsgdb CRDT + mesh_knowledge FNV + LayaIntent=8 fio | **PARTIAL**↑ | 0.5.0: `nos-mesh::crdt` LWW+VV host tests; LayaIntent TX/RX; **P2P NSGDB sync / lab 2-nó NOT** |
| M.3 | JARBAS HUD fleet + SoulMirror + lab 2+ nós | **PARTIAL**↑ | 0.5.0: `SoulMirrorSnapshot` + bus `SOUL_MIRROR`; lab 2-nó e2e **AWAITING_HW** |
| M.4 | capacity_weighted_assign offload S2 | **DONE** (host hint) | `assign_from_mesh_tuples` + `MESH_OFFLOAD_HINT` bus; remote infer e2e residual |

## Tema S1 / S2 / HW

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| S1.1 | Laya/Jev Decision Intent scorer CPU | **PARTIAL**↑ | `s1_backend_effective`: agentjev sem blob → keyword; `NOSD_S1_BLOB`/models/agentjev.gguf; LOO load metal AWAITING |
| S1.2 | Embutir scorer+Piper seed ISO warm mmap | **REJECT** | Política 2026-10-25: blobs externos **só runtime** via Cortex→Hermes→JARBAS (`blob_deliberate`); ISO NÃO embute |
| S1.2b | Runtime blob/firmware fetch deliberado | **DONE** | `nos-hermes::blob_deliberate` + `model.fetch`/`firmware.fetch`/`transmute` HITL; `iso.nix` comment |
| S2.1 | full-auto plan nvidia-smi/ROCm/DRM qualquer GGUF | **PARTIAL** / PREEXIST | capacity+plan pré-existente; ROCm thin |
| S2.2 | llama-server persistente stream+cancel | **PARTIAL**↑ | 0.5.0: `llama_server_posture()` honesty; SSE+cancel pré-existente; e2e daemon metal **NOT** |
| HW.1 | inventário tipado Hermes (udev/IIO/ACPI/…) | **PARTIAL** | = NX.3 thin |
| HW.2 | JARBAS cards HITL + SelfHeal firmware Nix | **PARTIAL**↑ | 0.5.0: `evaluate_self_heal` CapGate Deny-by-default + CardSpec; Slint ricos/firmware Nix **NOT** |

## Tema T / DB / F / A

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| T.1 | ModelHub 8 slots + LOO real + Trinity route | **PARTIAL** / PREEXIST | hub+route pré-existente; LOO nichos runtime residual |
| T.2 | AutoLearn Seek→fetch/treino/Factory→nsgdb | **DONE** (host) | `tick_autolearn` → `seek_before_spawn` (mesh+federated+OPAR); synth só se Seek exhaust; treino remoto residual |
| T.3 | FitPolicy/evict validate_for_slot | **PARTIAL** | `evict_coldest`/`ensure_loaded_fit` + testes hub; LOO metal/cap residual |
| DB.1 | nsgdb L2–L7 intensivo; zero JSONL | **DONE** (host default) | default `NOSD_MEMORY_BACKEND=nsgdb` → `L{2,3}/seg.bin` length-prefixed; L4 stub; JSONL só opt-in; MCP CRDT/BQ/L5–L7 = residual |
| DB.2 | OPAR Remember/Recall obrigatório todos loops | **PARTIAL**↑ | agency.tick + cognitive.tick amostrado (idle%32); AutoLearn/Sleep pré-existente; SelfHeal/mesh **NOT** |
| F.1 | Factory path A intensivo PackageHub | **DONE** (host) | Frugal wasmi path A; empty wasm → tiny identity; Seek-gated spawn |
| F.2 | SelfEvolve/SkillOpt | **PARTIAL** | `plan_skill_opt` + SelfEvolve AGENT.md bump; evolve loop pleno **NOT** |
| F.3 | Seek-Before-Generate mesh+GH/GL antes synth | **DONE** (host) | local→mesh→private→GH/GL/crates/MCP; NetAbsent honesty; `NOSD_SEEK_MOCK`; synth last; rate-limit residual |
| A.1 | Frota Agency EventDriven AGENT.md | **PARTIAL** | `tick_agency`→AGENCY_FLEET no Continuous; AGENT.md fleet pleno **NOT** |
| A.2 | Skills a quente Seek→import→wasmi | **PARTIAL** | `SkillObserver` + seek push nomes; wasmi import hot **thin** |

## Tema U / B / N / J / G

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| U.2 | k_ai Safety/Trust/Audit/SelfHeal/HWExpert nosd | **PARTIAL**↑ | 0.5.0: `evaluate_self_heal` Deny-by-default; `safety_trust` thin; HWExpert port **NOT** |
| U.3 | nos-bei budget/affect/sleep + SoulMirror | **PARTIAL** | BEI+Sleep; `tick_sleep_consolidate`→memória; SoulMirror cluster **NOT** |
| B.1 | nosd-early + BOOT SCORE + firstboot Slint | **PARTIAL**↑ | 0.5.x: early só `/var/lib/nosd`; firstboot fail-loud; ISO `NIXAIOS051`; metal journal AWAITING_HW |
| N.1 | /etc/nixos/ai-ring + generation-guard | **PARTIAL** | = NX.1 cortex-nix-ring |
| J.1 | nos-voice PipeWire full-duplex | **PARTIAL** / PREEXIST | 0.5.0: locale/piper paths Linux-first (sem FAT); EngineAbsent sem bins; duplex **thin** |
| J.2 | Generative Cards + Orb skill/hub | **PARTIAL** | `CardSpec` parse + WindowBroker; Slint generative rich widgets **NOT** |
| G.1 | Matriz/TECNOLOGIAS/addendum honesty | **DONE** | este ledger + SESSION_529 + TECNOLOGIAS N.3* |

## Tema P — Produtos (P.1–P.16)

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| P.1 | Cognitive Bridge + SOUL.md + BGE-RAG + IterationBudget | **PARTIAL** | `build_chat_prompt` SOUL+recall; BGE-RAG/IterationBudget **NOT** |
| P.2 | LatentBus [f16;256] Hermes↔Cortex↔Orb | **PARTIAL** | chat→`apply_latent`+`latent_tag`; f16[256] real **NOT** (f32 vec) |
| P.3 | AgentJev-0.6B backend S1 CapGate | **PARTIAL**↑ | gate efectivo + `guard-iso-no-s1-blob.sh` no build ISO; blob load runtime AWAITING |
| P.4 | whisper.cpp+Piper EngineAbsent off | **PARTIAL** | `VoiceEnginePosture::EngineReady` se CLI no PATH; sem bin = Absent honesto |
| P.5 | AirLLM layer-stream TooTight | **DONE** (host thin) | `run_layer_stream` + hub; llama mmap real residual |
| P.6 | Membrane + MCP + Cross-OS discover | **PARTIAL** | `MembraneGrant`; MCP bridge/discover live **NOT** |
| P.7 | Mesh crypto Noise/ML-KEM RESEARCH | **PARTIAL** | RESEARCH doc; ML-KEM **NOT** |
| P.8 | wasmi `aios_ai::infer` CapGate | **DONE** | host import + testes deny |
| P.9 | SleepCycle 5 fases consolidate nsgdb real | **PARTIAL** | Consolidate→`tick_sleep_consolidate`+SLEEP_CONSOLIDATE; nsgdb MCP real **NOT** |
| P.10 | Vision + RERANKER multimodal | **PARTIAL** | `vision_rerank` resolve keywords+EngineAbsent; blobs runtime **AWAITING** |
| P.11 | WWW/Browser suite CapGate | **PARTIAL** | `plan_www` only |
| P.12 | Marketplace assinado Ed25519 | **DONE** (host) | FNV + optional Ed25519 verify (`ed25519-compact`); trusted key ring fleet residual |
| P.12+ | Store federado índices | **DONE** (host) | `FederatedIndex` ingest Seek + RPC `store.index`; GPL→Pending; sandbox install gate |
| P.13 | FLEET_HEALTH + Hub triage + NOSD_SCORE | **PARTIAL**↑ | NosScore.summary+early_boot+s1+llama+landlock p/ HUD; triage LLM **NOT** |
| P.14 | Persona emotion/cache/heartbeats/VOICE_LATENCY | **PARTIAL** | `PersonaState` thin; cache 5-tier **NOT** |
| P.15 | BGE embeddings semantic recall | **PARTIAL** | `bge_embed` Absent+lexical fallback; semantic blob **NOT** |
| P.16 | Padrões assistd/Flowork/Anthropic Skills | **PARTIAL** | presence; docs patterns; Skills MCP **NOT** |

---

## Lição memorizada (anti-padrão)

**Marcar todo completed na UI sem cumprir o goal do plano = overclaim.**  
Próxima onda: só fechar item com (1) teste host ou (2) log runtime + linha neste ledger → DONE.

## Próxima onda sugerida (pós-0.5.0, prioridade honesty)

1. Metal stick `NIXAIOS051`: campanha [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) — Q5→transmute sem EACCES; cage tty7 (**AWAITING_HW** até journal anexado).  
2. Lab 2-nó fleet: [`FLEET_LAB.md`](FLEET_LAB.md) 0081 A/B + SoulMirror (M.3 residual).  
3. CRDT → sync P2P NSGDB (M.2 residual além do LWW in-process).  
4. AgentJev/BGE/Vision blobs via `blob_deliberate` LOO.  
5. llama-server e2e + AirLLM mmap real (S2.2).  
6. **S1.2 REJECT:** nunca seed ISO.  
7. Remote S2 infer no peer (M.4 hint só).  
8. Gate objectivo: `bash scripts/gate-boot-to-jarbas.sh` (stdout = ground truth IA).

## Referências

- Plano Cursor: `~/.cursor/plans/nixaios_remigração_34e29087.plan.md`  
- Playbook Boot→JARBAS: [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md)  
- `docs/memory/SESSION_529.md`  
- `nixaios/docs/adr-addenda/0118-remigration-nx.md`  
- `nixaios/docs/TECNOLOGIAS_NIXAIOS.md` (N.3*)  
- IDEA #707 (NixAIOS)
