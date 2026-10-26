# Roadmap — pós 0.5.0

Fonte de verdade item-a-item: [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md).  
Release notes: [`RELEASE_0.5.0.md`](RELEASE_0.5.0.md).

## Aceite 0.5.0 (fechado no código)

- Persistência Linux-first; mitigação EACCES pós-Q5.
- CapGate, S1.2 no-seed, Frugal, Seek-before-generate, autoSwitch=false.
- CRDT/SoulMirror/llama posture/SelfHeal stubs com testes host.

## Próxima onda (prioridade honesty)

1. **Metal** stick `NIXAIOS051` (`scripts/build-iso.sh`): Q5 → transmute → cage sem cascata permissão (AWAITING_HW).  
2. **Lab 2-nó** mesh e2e + SoulMirror wire.  
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
