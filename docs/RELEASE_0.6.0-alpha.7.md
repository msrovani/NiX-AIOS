# NixAIOS 0.6.0-alpha.7 — Ring Nix: decisão “desarmado” + kill switch no root

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.7  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.7`  
**Base:** 0.6.0-alpha.6

## Decisão (R-03)

O ring Nix **continua desarmado**. A implementação fica no tree (Cortex →
`nix.propose` → staging → helper root → `nixos-rebuild switch`), mas o
caminho só executa com `NOSD_NIX_RING=1` **e** `services.nosdNixRing.armRing =
true`. Critérios para religar:

1. S-04 completo (sem `hitl_confirmed` legacy).
2. R-03/R-04 verdes com teste VM de apply **e** rollback (mudar hostname e
   reverter) em `nixaios-vm`.
3. ADR assinada + kill switch documentado em `docs/SECURITY.md`.

Enquanto isso, `adr-addenda/0118-nix-ring-decision.md` registra a decisão.

## R-04 — o helper root deixou de confiar no pedido

Antes: `/var/lib/nosd` é **1777** na ISO; qualquer user podia escrever
`nix-apply.request` + staging e o `nosd-nix-apply.service` (root) copiava para
`/etc/nixos/ai-managed.nix` e corria `nixos-rebuild switch`. O `target` vinha
do próprio pedido.

Agora (`nosd-nix-apply` e `nosd-nix-rollback`):

| Antes | Agora |
|---|---|
| `target=` vindo do pedido | **fixo** `/etc/nixos/ai-managed.nix` |
| `staging=` em qualquer path | tem de estar **sob** `$NOSD_DATA_DIR` |
| sem lock | `flock -n` em `$DATA/nix-apply.lock` |
| `nixos-rebuild` sem limite | `timeout 900` (switch) / `600` (rollback) |
| PATH só com nix/nixos-rebuild | + `coreutils` (`date/chown/cat/grep/...`) |
| sem kill switch | `NOSD_NIX_RING != 1` → **ignora e remove o pedido** |

## Teste no nível do bug (VM)

`checks.x86_64-linux.nixaios-vm` passa a importar `cortex-nix-ring` com
`armRing = false` e faz:

```
echo 'staging=/var/lib/nosd/pwn.nix' > /var/lib/nosd/nix-apply.request
systemctl start nosd-nix-apply.service || true
test ! -e /etc/nixos/ai-managed.nix
test ! -e /var/lib/nosd/nix-apply.request
```

Isto é o que um teste unitário de Rust não consegue provar: PATH do systemd,
permissões e o oneshot real.

ISO: SHA-256 `88e88083b812031243d47adfc4df7f5707a4a5c40d428f65626732edffea7403`;
`stat` no guest: `660 nosd:nosd`; `NOSD_NIX_RING` ausente do unit (0);
gate MQEMU PASS; VM test PASS com o pedido forjado ignorado.
Evidencia: `docs/evidence/NIX_RING_0.6.0-alpha.7.md`.

## Honestidade

- **R-03/R-04 não estão “feitos”**: o ring está desarmado por decisão, não por
  estar seguro. A feature continua sem teste de apply/rollback em VM.
- QEMU/VM ≠ metal; M4/M5 seguem AWAITING_HW.
- O `armRing` é um interruptor de código; quem tiver root no host consegue
  sempre editar a unit — isto protege contra o utilizador não-privilegiado em
  `/var/lib/nosd` 1777, não contra root.
