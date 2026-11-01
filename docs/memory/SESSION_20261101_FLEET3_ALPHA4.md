# Sessão 2026-11-01 — 0.7.0-alpha.4 frota-3 + metal 240H

## Entregue in-tree
- ISO canónica `iso-hw-cuda`: S1 0.8B Q4 + S2 4B Q6_K; Landlock pós-spawn.
- Mesh: broadcast subnet, UDP 19000, node-id de machine-id; ENETUNREACH backoff.
- `brain.adapt` HITL up/down/driver_hint; tier4 hot_load VRAM sem ring.
- Cage dbus/PipeWire/espeak; ORB 2K/cam/idle; `boots/<stamp>_<host>/` + LATEST.
- `iso-hw-cuda-legacy` (driver 580) opcional Pascal.
- `pack-hw-usb.sh`: `mv` no mesmo FS (não duplicar 20 GiB no VHDX WSL).

## Artefactos host (WSL)
| Artefacto | sha256 (prefix) | notas |
|-----------|-----------------|-------|
| ISO `iso-hw-cuda` ~17 GiB | `361cfa649131758e…bc26f29f` | `result/iso/nixos-minimal-…iso` |
| `.img` híbrido ~21 GiB | `fa51efd5fa712202…5ea454a8` | `/home/msrov/nixaios-hw-cuda-0.7.0-alpha.4.img` |

Gravar **DD** (Rufus DD / `dd`), nunca modo ISO.

## Metal Core 7 240H + RTX 3050 (foto HUD 2026-11-01)
**OBSERVED (parcial, sem log stick lido nesta máquina):**
- ORB cinematic LIVE; S2 `llama-server-http` :8082 a responder; GPU ~4235/6144 MiB.
- Landlock Verified; CAM ON; chat texto OK (“responde bem”).
- MIC OFF; sem saída de som (STT/TTS/PipeWire ou DeviceAllow).
- peers=0 (frota ainda não meshed nesta observação).
- Recusa alterar Nix: **esperado** com `armRing=false` → `NIX_RING_DISABLED`.
  Auto-melhoria = `nix.propose` → HITL → `nix.confirm`, não `nixos-rebuild` directo;
  falta wiring de tool/skill no chat (persona sozinha não chega).

**Not-tested / bloqueios host:**
- Partição `NIXAIOSLOG` (FAT32 append) no Windows: **Offline** (D/E);
  UAC cancelado → `boots/LATEST` não copiado. Precisa Online em diskmgmt ou
  `wsl --mount` elevado.
- Mesh 3 nós, HITL adapt cards, áudio ponta-a-ponta, ring armado lab.

## Armadilhas desta sessão
1. `tr -d '\r'` em `.nix`/`*.rs` apaga a letra **r** (`return`→`retun`, `warn`→`wan`).
2. BOM UTF-8 no `flake.nix` parte o eval Nix.
3. Pack com `dd` WORK→OUT no mesmo VHDX enche C: (RO remount / WSL DISK_FULL).
4. Rufus elevado: Computer Use / rufus.com sem UIPI não clicam a UI admin.
5. Hybrid `.img`: Windows vê EFI+FAT Offline até Online manual / mount WSL.

## Próximo
1. Online `NIXAIOSLOG` → copiar `boots/*_240H*` → fechar áudio vs journal.
2. Skill/persona + tool `nix.propose` no ORB; lab `armRing=true` só com ADR.
3. Re-boot frota (i5/1050 + 7700G) e mesh peers≥2.
