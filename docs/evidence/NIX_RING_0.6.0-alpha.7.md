# Evidence — Ring Nix desarmado / kill switch root (0.6.0-alpha.7)

**Data:** 2026-10-27  
**Ambiente:** `checks.x86_64-linux.nixaios-vm` — **OBSERVED** (VM, não metal)  
**Decisão:** ADR addendum `docs/adr-addenda/0118-nix-ring-decision.md`

## Teste no nível do bug

O VM test importa `cortex-nix-ring` com `armRing = false` (default) e escreve
um pedido de apply como qualquer user poderia fazer em `/var/lib/nosd` (1777
na ISO):

```
echo 'staging=/var/lib/nosd/pwn.nix' > /var/lib/nosd/nix-apply.request
systemctl start nosd-nix-apply.service || true
test ! -e /etc/nixos/ai-managed.nix
test ! -e /var/lib/nosd/nix-apply.request
```

Log da VM:

```
systemd[1]: Starting Apply nosd staging -> ai-managed.nix + nixos-rebuild switch...
systemd[1]: nosd-nix-apply.service: Deactivated successfully.
systemd[1]: Finished Apply nosd staging -> ai-managed.nix + nixos-rebuild switch.
machine: (finished: must succeed: test ! -e /etc/nixos/ai-managed.nix, in 0.22 seconds)
machine: (finished: must succeed: test ! -e /var/lib/nosd/nix-apply.request, in 0.26 seconds)
```

`/var/lib/nosd/nix-apply.log` (guest) contém:

```
ring Nix desarmado (NOSD_NIX_RING != 1) - pedido ignorado
```

## Endurecimento aplicado

| Item | Antes | Depois |
|---|---|---|
| Kill switch | nenhum (só o nosd não escrevia) | helper root exige `NOSD_NIX_RING=1` |
| `target=` | vinha do pedido | fixo `/etc/nixos/ai-managed.nix` |
| `staging=` | qualquer path | tem de estar sob `$NOSD_DATA_DIR` |
| concorrência | nenhuma | `flock -n` |
| duração | ilimitada | `timeout 900` (switch) / `600` (rollback) |
| PATH | nix + nixos-rebuild | + coreutils (`date/chown/cat/grep/cut/...`) |
| rollback | livre | mesma gate do ring |

## Honestidade

- Isto prova que o caminho root **não** aceita um pedido forjado por um
  utilizador não-privilegiado com o ring desarmado.
- **Não** prova R-03/R-04 verdes: falta o teste de apply/rollback real
  (mudar hostname e reverter) com `armRing = true`.
- Quem tem root no host contorna o interruptor (edita a unit); o modelo de
  ameaça aqui é o user em `/var/lib/nosd` 1777.
