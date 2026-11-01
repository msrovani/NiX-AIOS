# Evidência 0.7.0-alpha.4 — AIOS frota-3

**Status:** in-tree + ISO/img host OBSERVED. Metal 240H **parcial** (HUD); logs stick
AWAITING (partição NIXAIOSLOG Offline no Windows). Frota 3 nós AWAITING_HW.

## Norte
Uma ISO (`iso-hw-cuda`): S1 `Qwen3.5-0.8B-Q4_K_M` + S2 `Qwen3.5-4B-Q6_K` `:8082`.
Máquina só muda capacidade; HITL para mudar GGUF/driver.

## Frota
| Nó | Link | Cage | Infer expectativa |
|----|------|------|-------------------|
| Core 7 240H + RTX 3050 | Wi‑Fi | i915 | CUDA S1+S2 — **HUD OBSERVED 2026-11-01** |
| i5 + HD620 + GTX 1050 | Wi‑Fi | i915 | CPU (595 ignora Pascal); HITL downgrade/driver |
| Ryzen 7700G + Vega + 4070 Super | **wired** | amdgpu | CUDA; HITL upgrade 9B se VRAM folga |

Mesh: UDP 19000 aberto no HW; broadcast subnet (Wi‑Fi↔eth mesma LAN); `node_id` de machine-id.

## Artefactos
| Item | Bytes (approx) | sha256 |
|------|----------------|--------|
| ISO `iso-hw-cuda` | 17 221 955 584 | `361cfa649131758e95ee202a6811408f6fb5131859b562e0635a004ebc26f29f` |
| `.img` híbrido (ISO+FAT 4 GiB) | ~21 GiB | `fa51efd5fa7122025e47f7019a7f2134865315cc7fa1f33bfc46c5895ea454a8` |

Gravar o `.img` em **DD** (Rufus DD / `dd`). Memória de sessão:
[`docs/memory/SESSION_20261101_FLEET3_ALPHA4.md`](../memory/SESSION_20261101_FLEET3_ALPHA4.md).

## Mudanças in-tree
- Landlock **depois** do spawn llama (CUDA filhos).
- `brain.adapt` bidirecional + kinds HITL; tier4 hot_load com VRAM sem ring.
- Cage: dbus-launch + PipeWire + espeak saudação.
- ORB: `--fs` 2K, webcam 4:3, idle shield, DIAG S2 fallback honesto.
- NIXAIOSLOG: `boots/<stamp>_<host>/` + `LATEST`.
- `pack-hw-usb.sh`: rename no mesmo FS (anti-DISK_FULL no VHDX WSL).
- Opcional: `iso-hw-cuda-legacy` (580) para CUDA no 1050.

## Metal 240H (HUD, sem journal stick)
- LIVE / CORE NOMINAL; S2 HTTP :8082; GPU ~4.2/6.0 GiB; Landlock Verified; CAM ON.
- MIC OFF; sem áudio de saída; peers=0.
- Chat texto OK; recusa Nix = ring desarmado (`armRing=false`) — correct fail-closed.
- Stick: partições D/E Offline no host Windows → `boots/` não lido nesta sessão.

## Tested
- `nix build .#iso-hw-cuda` (WSL) → ISO acima.
- `scripts/pack-hw-usb.sh` → `.img` acima (OUT em ext4 WSL, `mv` same-FS).
- Unit: `nos-core::net_util`, brain adapt / tier4 VRAM; TOFU settle +1.5 s margem.

## Not-tested
- Journal `boots/*` do 240H (mount NIXAIOSLOG).
- Áudio/STT/TTS ponta-a-ponta; mesh peers≥2; i5/1050 + 7700G.
- `iso-hw-cuda-legacy`; `armRing=true` lab; tool `nix.propose` no ORB chat.
