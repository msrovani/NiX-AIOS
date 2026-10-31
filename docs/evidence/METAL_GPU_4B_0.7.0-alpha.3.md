# Metal GPU + ISO 4B — 0.7.0-alpha.3 (2026-10-31)

## Summary

Correções metal iGPU/dGPU (logs `NIXAIOSLOG` 2026-10-31) e variante
**iso-hw-cuda-4b** (só `Qwen3.5-4B-Q6_K`).

## Causes (pre-fix, OBSERVED)

Ver [`hw/2026-10-31_1944Z/README.md`](hw/2026-10-31_1944Z/README.md):

- A: `nvidia-smi` SIGSEGV — `.dynamic` em `PT_LOAD` só `PF_R` (patchelf + glibc 2.44)
- B: Chromium → Slint — `HOME=/var/empty`, Crashpad, `NoNewPrivileges` vs SUID, GLVND NVIDIA
- C: S1 `NGL=0` + `plan_route(model_prompt)` (SOUL) ⇒ S2Deep
- D: capacity probe antes de `nvidia-drm` (`cards=["card1"]`)

## Fixes (in-tree)

| Área | Onde |
|---|---|
| ELF PF_W | `scripts/fix-nvidia-elf-pf-w.py`, `images/hw-cuda.nix`, check `nvidia-smi-elf-pf-w` |
| Cage/Chromium | `modules/jarbas-cage.nix` (HOME/XDG, Crashpad, NNP, Mesa pin) |
| S1+S2 CUDA | `images/hw-cuda.nix` (`NGL=99`, wait nvidia) |
| Capacity/rota | `nos-cortex::capacity`, `nos-hermes` `plan_route(user)`, `slot_map` active* |
| ISO só 4B | `images/hw-4b.nix`, `images/hw-cuda-4b.nix`, `.#iso-hw-cuda-4b` |

## Artefactos 4B (host Downloads)

| Ficheiro | Bytes | sha256 |
|---|---|---|
| `nixaios-hw-cuda-4b.iso` | 7 648 026 624 | `67355245e1aa59bd927e7b54a89bc88203150b2e6beb275ec7f0bce4bedb4a97` |
| `nixaios-hw-cuda-4b-logfat.img` | 9 795 796 992 | `5bacea853211b129f4bc2ddce966a4266670bf05ae4865fe16f6706b79eae52d` |

Gravar o **`.img` em DD**. Modelo embutido: `active.gguf` → 4B Q6_K; sem S2 :8082.

## Tested

- `cargo test` nos-cortex (slot_map/capacity), nos-hermes (`plan_route_uses_user`), nosd lib
- `checks.x86_64-linux.nvidia-smi-elf-pf-w` PASS; `hw-cuda-deviceallow` PASS
- `nix build .#iso-hw-cuda-4b` PASS (retry pós Bus error xorriso / WSL low RAM)

## Not-tested

- Metal re-boot com esta ISO (cinematic estável, smi sem ERRO, 4B em VRAM)
- QEMU cinematic

Tested: host checks + ISO build. Not-tested: metal.
