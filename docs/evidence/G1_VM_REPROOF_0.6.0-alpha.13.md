# Evidência — re-prova VM do `nixaios-vm` com as tranches G0–G5

**Data:** 2026-10-29 · **Estado:** OBSERVED (1 execução; repetição pendente)
**Derivação:** `/nix/store/cj4vhz6y0grp67vj1qmp56j6v46i7vck-vm-test-run-nixaios-vm.drv`
**Check:** `check.x86_64-linux.nixaios-vm` (tempo de script: 172 s).

## O que passou (destaques + Novidade)

1. Boot → `nosd.service` → socket 0660 → `health` ✓ (posture honesto).
2. Chat GGUF (`llama-server-http`) → unload → `cpu-echo` → load ✓.
3. `accept-m3` ✓ · `voice-status Ready` ✓ · voice-tts WAV ✓.
4. **Ring desarmado por omissão**: pedido forjado ignorado, staging não
   aplicado ✓ (assertion negativa do alpha.7 mantida).
5. **G1 (regressão lançada nesta tranche), com identidade activo no VM**
   (`NOSD_MESH_KEY` + `REQUIRE_SIG`):
   - `systemctl restart nosd` → reabrir com **âncora assinada** (`NSGANCHOR`)
     — **continua a abrir** (fail-closed correcta, sem falso rollback) ✓
   - `wc -w < seg.anchor` = **3 campos** (assinada) ✓
   - `/var/lib/nosd/keys/pins.dat` existe (pins TOFU persistidos) ✓
   - `/var/lib/nosd/keys/hb.seq` não vazio (piso do seq gravado no arranque) ✓
6. Journal: landlock **`Verified`** no arranque do daemon ✓ (T1.5).

## Honesty

- Sem observação de **QEMU/METAL cisar nesta evidência**: nixaios-vm
  (NixOS tester) ≠ QEMU ISO ≠ metal — o ciclo `boot→firstboot→cage` segue
  controle substancial separado (`docs/evidence/BOOT_FIRSTBOOT_*`).
- O RX fetch real do brain-upgrade (tier 2/3) e o meu pulse to-history
  pós-boot NOSD_BRAIN_HELP ainda **verificação de máquinas/iso-hw** (AWAITING verify
  runtime e2e).
- VERIFIED (3×) pendente; CI externo (GitHub) a correr sobre este MACRO da
  prova a navegação.

## Cross-checks desta onda (mesma proveniência)

- `nixaios-ring` (two-phase, stub rebuild): OBSERVED — ver
  `NIX_RING_TWO_PHASE_0.6.0-alpha.13.md`.
- Bench host: `cargo test --workspace` verde; clippy `-D warnings` zero.
