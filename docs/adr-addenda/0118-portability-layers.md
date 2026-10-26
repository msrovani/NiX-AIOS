# Addendum ADR-0118 — Camadas de portabilidade (bare-metal → NixOS)

**Status:** Accepted  
**Data:** 2026-10-23  
**Parent:** ADR-0118  
**Matriz viva:** [`../MIGRATION_MATRIX.md`](../MIGRATION_MATRIX.md)

Sob NixOS o substrato é **Linux** (não Ring0 próprio). O catálogo `TECNOLOGIAS.md` §1–15 divide-se em três níveis.  
Regra ADR-0113: **portável em papel ≠ portado no `nosd`**.

## 1. PORT — aplicação / agentes / runtimes cognitivos

Agnósticos do OS; alvo = daemons / WASM / crates `std` no flake.

| Tecnologia (catálogo) | Disposition | NixAIOS hoje (honesty) |
|-----------------------|-------------|-------------------------|
| Agent ontológico / Hermes | PORT | `nos-hermes` OPAR + Continuous — host VERIFIED |
| CapGate + Factory path A (wasmi) | PORT | Frugal 256 KiB / fuel 100k — **wasmi**, não wasmtime |
| neural-sgdb dual-mode | PORT | `nos-memory` facade FileStorage — WSL OBSERVED |
| Mesh P2P / Brain Mesh | PORT | UDP HB — wired; lab 2-node UNKNOWN |
| Trinity MoE / Cortex BitNet / Falcon3 | PORT *(contrato)* | **score_v0 + HotSwap llama/GGUF** — ≠ BitNet ternário no daemon; W2A8/AVX2 bare = BAREMETAL path |
| BEI / SleepCycle / SelfEvolve | PORT *(lógica)* | `nos-bei` tick **scaffold**; SleepCycle bare **não** portado 1:1 |
| Safety / Cognitive Bridge | PORT | CapGate + NOSD_SCORE — subset; Asimov/Merkle pleno = residual |

## 2. ADAPT — UI / áudio / E/S (POSIX)

Lógica de domínio mantém-se; hardware path troca para Linux.

| Tecnologia | Disposition | NixAIOS hoje |
|------------|-------------|--------------|
| JARBAS / Cards / Orb | ADAPT | **Wayland + `cage` → DRM `card0`** — **não X11** (addendum kiosk) |
| Áudio Piper/STT/VAD | ADAPT → DELEGATE | `nos-voice` EngineAbsent; PipeWire/ALSA = F7 |
| NeuralFS / FAT / exFAT | ADAPT / DELEGATE | models dir + ext4 host; FUSE NeuralFS = **RESEARCH** |
| HotSwap / HF fetch GGUF | ADAPT | unload/hot_load + ureq CapGate NET\|FS (s507); EXL2/vLLM Absent |
| GPU compute | ADAPT | dGPU via CUDA/HIP flake opt-in; BAR Mapped bare ≠ nosd |

## 3. BAREMETAL_ONLY / DELEGATE_LINUX — redundante sob NixOS

Linux assume o silício; não reimplementar no `nosd`.

| Classe | Exemplos | Disposition |
|--------|----------|-------------|
| Boot / IRQ / paging | Limine, IDT, IST, bitmap PMM, #PF manual | BAREMETAL_ONLY |
| Drivers NIC/storage/USB | e1000, i225, NVMe MMIO, AHCI, ATA PIO, xHCI | DELEGATE_LINUX (`igc`, `nvme`, …) |
| SMP trampoline / Ring3 bare | SIPI, iretq CPL3 | BAREMETAL_ONLY / SUPERSEDE_NIX (cgroup/seccomp) |
| CFS próprio / TicketLock IRQ | — | DELEGATE_LINUX / subset `std` |

## Vantagens Nix (declarativo)

1. **Flake hermético** — deps + (futuro) blobs `.bitnet`/GGUF pinned.  
2. **Sandbox** — CapGate + systemd + Landlock/seccomp (Ring3 SUPERSEDE).  
3. **Imutabilidade** — gerações Nix + HITL `ai-managed` (staging ≠ `switch` automático).

## Anti-claims (deny)

- “Cortex Falcon3 BitNet já roda nativo no nosd” → **falso** até PORT real do crate `cortex` (hoje = llama GGUF HotSwap).  
- “UI via X11 OK na ISO” → **falso** (Wayland+cage only).  
- “wasmtime no path A” → **falso** (wasmi Frugal).  
- “ISO build PASS = metal PASS” → **falso** (0113).
