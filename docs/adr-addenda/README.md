# ADR addenda — NixAIOS disposition

Hub geral da documentação: [`../README.md`](../README.md).  
Addenda curtas por cluster. Corpos históricos bare-metal **não** são reescritos.

| Cluster | ADRs | Disposicao NixAIOS |
|---------|------|-------------------|
| Premissa | 0088, 0113, 0106 | PORT doutrina; evidência = journald/ISO |
| Cortex / modelos | 0078, 0101, 0112, 0057 | PORT hub + ADAPT HotSwap/GPU |
| Hermes / WASM | 0059, 0052, 0041 P3 | PORT Factory A + CapGate |
| BEI | 0060 | PORT tick fino; não substitui LLM |
| Mesh | 0081 | PORT protocolo UDP std |
| Memory | 0091 | PORT neural-sgdb FileStorage |
| UI | 0058, 0090 | ADAPT Slint + **Wayland cage** (`0118-wayland-cage-kiosk.md`); **persona JARBAS** (`0118-jarbas-persona-ui.md`) — janelas HITL |
| Portabilidade | 0118 | Camadas PORT/ADAPT/DELEGATE — `0118-portability-layers.md` |
| HotSwap/HF | 0118 | GGUF llama MVP — `0118-hotswap-hf.md` |
| Remigration NX.0 | 0118 | Honesty ISO + HITL Nix ring — `0118-remigration-nx.md` |
| Ring Nix (R-03/R-04) | 0118 | **Desarmado por decisão** + kill switch no helper root — `0118-nix-ring-decision.md` |
| Remigração ledger | 0118 / #707 | **55 itens** DONE/PARTIAL/NOT_DONE — [`../REMIGRATION_PLAN_LEDGER.md`](../REMIGRATION_PLAN_LEDGER.md) (SESSION_529) |
| Install | 0086 | SUPERSEDE_NIX (ISO + rebuild) |
| Ring3 | 0077, 0102 | SUPERSEDE_NIX (namespaces/seccomp) |
