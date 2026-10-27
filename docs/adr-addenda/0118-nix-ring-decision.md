# ADR-0118 addendum — Ring Nix: desarmado por decisão (R-03/R-04)

**Estado:** aceite (2026-10-27, 0.6.0-alpha.7)  
**Substitui:** nada. **Relaciona:** S-04, S-05, S-06, R-01, R-03, R-04.

## Contexto

O ring Cortex→Nix permite que Hermes/Cortex proponham configuração NixOS que
um helper root aplica (`nosd-nix-apply.service` → `/etc/nixos/ai-managed.nix` →
`nixos-rebuild switch`). Na ISO live, `/var/lib/nosd` é `1777` para o
firstboot/nosd escreverem, e o pedido é apenas um ficheiro.

Risco: um utilizador não-privilegiado podia escrever `nix-apply.request` e um
staging → root aplicava Nix arbitrário. É exatamente o que a regra “nenhum
caminho root sem autenticação” proíbe.

## Decisão

1. O ring fica **desarmado** (`services.nosdNixRing.armRing = false`, default).
   O código permanece no tree, inerte.
2. Os helpers root **recusam** qualquer pedido sem `NOSD_NIX_RING=1`, apagam o
   `request` e registam em log (fail-closed).
3. Mesmo armado, o helper:
   - ignora `target=` do pedido (fixo `/etc/nixos/ai-managed.nix`);
   - exige `staging` dentro de `$NOSD_DATA_DIR`;
   - usa `flock`, `timeout` e PATH explícito (coreutils, nix, nixos-rebuild).
4. Rollback segue a mesma regra (um rollback forçado é uma regressão de
   segurança).

## Critérios para religar (armRing = true)

- S-04 completo: grants server-authored em **todos** os caminhos admin, sem
  `hitl_confirmed` legacy.
- R-03/R-04 verdes: teste VM que muda `networking.hostName`, aplica, verifica
  e faz rollback, com `build-vm` antes do switch.
- `docs/SECURITY.md` com o fluxo, o kill switch e o procedimento de incidente.
- Revisão explícita: o pedido não pode ser escrito por utilizadores fora do
  grupo `nosd` (idealmente `0750 nosd:nosd` em `/var/lib/nosd` no produto).

## Consequências

- O produto **não** auto-configura NixOS a partir do chat; quem quiser isso usa
  `nisctl nix-propose` + `nixos-rebuild` manual (root consciente).
- M3 (Boot→JARBAS) não depende do ring.
- Se o ring for religado sem os critérios, o teste da VM
  (`test ! -e /etc/nixos/ai-managed.nix` com `armRing=false`) falha primeiro.
