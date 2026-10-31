# Metal HW evidence — 2026-10-31 ~19:44Z (pre-fix logs)

Logs copiados do FAT32 `NIXAIOSLOG` (USB) para diagnóstico do iso-hw-cuda
(RTX 3050 + Intel RPL-P). **Não inclui** `wifi.conf` / credenciais.

## Tested (esta pasta = OBSERVED no boot antigo)

- Cage `pickDrm` → `/dev/dri/card1` (i915), eDP-1 2560×1600 @ 120 Hz.
- `nosd-engine-s2` sobe; porta 8082 responde em algum instante do boot.
- `nvidia-smi` **SIGSEGV** (coredump user 998/0); DIAG `GPU · smi ERRO`.
- Chromium cinematic → Crashpad `--database is required` → fail-over Slint
  (`winit-software` / pixman).
- Capacity boot: `cards=["card1"]`, `vram_free_mb=0`, `posture=nvml_absent`,
  `NGL 0` (forçado em `hw-cuda.nix` pré-fix).

## Causas raiz (resumo)

| ID | Defeito |
|---|---|
| A | ELF `nvidia-smi`: `.dynamic` em `PT_LOAD` só `PF_R` (patchelf) → glibc 2.44 |
| B | Chromium: `HOME=/var/empty` + Crashpad; `NoNewPrivileges` vs SUID; GLVND NVIDIA |
| C | S1 `NOSD_LLAMA_NGL=0` + `plan_route(model_prompt)` (SOUL) ⇒ S2Deep |
| D | Probe DRM antes de `nvidia-drm` registar `card0` |

## Not-tested (honesto)

- Rebuild `iso-hw-cuda` com os fixes desta tranche + metal re-boot.
- Chromium cinematic estável na iGPU Mesa pós-HOME/NNP/Mesa pin.
- S1+S2 ambos em CUDA com VRAM medida por `nvidia-smi` patched.
- QEMU cinematic (continua AWAITING).

## Correções in-tree (sessão seguinte a estes logs)

Ver plano *Metal GPU Corrections*: `scripts/fix-nvidia-elf-pf-w.py`,
`images/hw-cuda.nix`, `modules/jarbas-cage.nix`, `nos-cortex::capacity`,
`nos-hermes` `plan_route(prompt)`, check `nvidia-smi-elf-pf-w`.
