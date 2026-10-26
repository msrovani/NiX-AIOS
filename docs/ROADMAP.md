# Roadmap — pós 0.5.0

Fonte de verdade item-a-item: [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md).  
Release notes: [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md) / [`RELEASE_0.5.2.md`](RELEASE_0.5.2.md) / [`RELEASE_0.5.4.md`](RELEASE_0.5.4.md) / [`RELEASE_0.5.5.md`](RELEASE_0.5.5.md).  
**Trajetória boot→JARBAS:** [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md) (M0–M5 + rings + AI_GATE). Gate: `scripts/gate-boot-to-jarbas.sh`.

## Aceite 0.5.0 (fechado no código)

- Persistência Linux-first; mitigação EACCES pós-Q5.
- CapGate, S1.2 no-seed, Frugal, Seek-before-generate, autoSwitch=false.
- CRDT/SoulMirror/llama posture/SelfHeal stubs com testes host.

## Próxima onda (prioridade honesty)

0. **M3 bancada** — fechar gaps Jarvis-like host/ISO ([`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md) M3a–f).  
1. **M4 Metal** stick `NIXAIOS051` — campanha [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) (AWAITING_HW até journal).  
2. **M5 Lab 2-nó** — [`FLEET_LAB.md`](FLEET_LAB.md) mesh e2e + SoulMirror (0081 A/B).  
3. **CRDT → P2P NSGDB** (além do LWW in-process).  
4. Blobs AgentJev/BGE/Vision via `blob_deliberate` + LOO (sem seed ISO).  
5. **llama-server** e2e + AirLLM mmap real.  
6. Remote S2 infer no peer (hoje só `MESH_OFFLOAD_HINT`).  
7. CUDA flake `.#llama-cpp-cuda` VERIFIED com offload.

## Non-goals (explícitos)

- Reimplementar boot/IDT/PMM/drivers bare no flake.
- Embutir modelos na ISO.
- X11 no kiosk.
- Marcar VERIFIED sem evidência metal/lab.
- wasmtime como path A Factory.

## Como actualizar este roadmap

Ao fechar um residual: (1) teste ou journal, (2) linha no ledger → DONE/PARTIAL↑, (3) entrada no CHANGELOG, (4) uma frase aqui.
