# Remigração NIXAIOS — aceite das ondas do plano (c2d76600)

**Não edita o plan file.** Fonte de gaps: `REMIGRATION_PLAN_LEDGER.md`.  
Taxonomia ADR-0113: host VERIFIED ≠ metal VERIFIED.

| Todo plano | Entrega código | Aceite host | Residual honesty |
|------------|----------------|-------------|------------------|
| I.1 CapGate server-authored | `CapRegistry` + testes | **VERIFIED** | — |
| M.1 Mesh FRACK/TOFU/election | `frack` + `tofu_settled` + election CapacityScore | **VERIFIED** unit | lab 2-nó e2e |
| DB.1 nsgdb intensivo | `nsgdb/L{2,3}/seg.bin` length-prefixed (não JSONL default) | **VERIFIED** unit | MCP CRDT/BQ; L4 stub |
| T.1 ModelHub 8 slots + AutoLearn | `ModelSlot::ALL` 8 + `tick_autolearn` | **VERIFIED** | LOO nichos metal |
| F.1 Factory Frugal + Agency | `AppFactory` wasmi + `AgencyFleet` | **VERIFIED** | Path B/C Absent |
| S1 Laya/Jev | keyword S1 + `S1_SCORER_ABSENT` LOO | **PARTIAL→OK plan** | blob Jev runtime |
| S2 dGPU + llama-server | capacity plan + `LlamaServerBackend` SSE | **PARTIAL→OK plan** | e2e daemon |
| P.1–P.9 | Bridge, LatentBus, budget, membrane, infer, Sleep, AirLLM plan, voice posture | **PARTIAL→OK plan** | BGE-RAG pleno |
| P.10–P.16 | vision/www/store hash+sandbox/fleet/persona/BGE Absent/patterns | **PARTIAL→OK plan** | blobs + Ed25519 sig |
| G.1 Matriz/TECNOLOGIAS | ledger + SESSION_529 + este ficheiro | **DONE** | — |

**Política blobs:** S1.2 seed ISO = **REJECT**; runtime `blob_deliberate` only.
