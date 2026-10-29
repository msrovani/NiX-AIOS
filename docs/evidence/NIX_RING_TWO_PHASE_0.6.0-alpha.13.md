# Evidência — `nixaios-ring` (two-phase do ring Nix, helper ARMADO)

**Data:** 2026-10-29 · **Estado:** OBSERVED (1 execução; VERIFIED = 3×, pendente)
**Derivação:** `/nix/store/f1k754l0h444xp9xz4i4jw7hqg129m4v-vm-test-run-nixaios-ring.drv`
**Check:** `check.x86_64-linux.nixaios-ring` (`pkgs.testers.nixosTest`, 1 VM,
ring `armRing=true`, `twoPhase=true`, `buildVmFirst=false`, stub `nixos-rebuild`).

## O que o teste prova (fases 1→5)

1. **Pedido forjado sem staging válido → recusa** (ring ARMADO): `FAIL missing
   staging`, REQ removida, nada escrito em `/etc/nixos/ai-managed.nix`.
2. **Proposta por FIELDS tipados (T3.1):** `nosctl nix-propose --fields
   '{"hostname":"ring-vm"}'` → staging do template fixo em
   `/var/lib/nosd/staging-ai-managed.nix` com `networking.hostName`.
3. **HITL + fase 1:** `nosctl consent nix.apply` → grant server-authored
   (kind `nix.apply`, single-use) → `nosctl nix-apply --confirm-id <id>` →
   copy → `nixos-rebuild test` (stub) → **aguarda nix.confirm**; stub log tem
   exactamente 1× `rebuild test` e **0×** `rebuild switch`; `nix-apply.request`
   consumo feito; `/etc/nixos/ai-managed.nix` = staging.
4. **Confirm expirado → recusa:** `ts=1` fake → `FAIL confirm expirado`
   (TTL 600 s), sem switch, files limpos.
5. **Fase 2 via RPC:** `consent` → `nosctl nix-confirm --confirm-id <id>`
   → `nixos-rebuild switch` (stub) → `switch OK` → **`last-generation.txt`
   não vazio**.

## Bugs latentes que este check apanhou (e fixos)

1. `gnugrep` ausente no `path` do unit `nosd-nix-apply` → exit 127 com ring
   ARMADO antes da fase 1 (`05418d3`).
2. Double-consume do grant HITL no gate Confirm + handlers (`nix.apply`,
   `wifi.connect`) — "HITL required" com grant válido (`00be264`); kind
   próprio `nix.build_vm` no gate.
3. Kill switch do daemon desligado mesmo com `armRing=true` no helper
   (`NOSD_NIX_RING=1` agora exportado ao unit nosd; em HEAD via `8681e09`).
4. Diagnóstico que outrora mostrou `last-generation.txt` vazio em 1 run
   (não-reprodutível na repetição verde; vigilante em futuras repetições).

## Honesty

- `nixos-rebuild` é **STUB**: prova o protocolo do helper (fases, TTL,
  negativos, gen-file), NÃO o rebuild real/binary closure (TCG ⓘ).
- VERIFIED exigirá 3 execuções + corrida no CI externo (GitHub, Q-03) e
  repeat com rebuild real em metal/tester com nixpkgs resolúvel.
