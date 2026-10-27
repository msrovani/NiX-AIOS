# NixAIOS 0.6.0-alpha.8 — S-04 fechado (HITL só por grant)

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.8  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.8`  
**Base:** 0.6.0-alpha.7

## O que muda

O booleano `hitl_confirmed` **deixou de existir como autorização**. A única
prova aceita é um `confirm_id` emitido pelo servidor (`ui.action` sobre uma
janela `hitl_confirm`), single-use, kind-bound e com TTL de 5 min.

### Servidor

- `hitl_grant_ok()` só aceita `confirm_id`; **nem root/token** (LocalAdmin)
  converte `hitl_confirmed` em consentimento.
- O gate de política (`SITE_POLICY escalate` / `confirm`) passou a usar
  `hitl_grant_ok` com `grant_kind_for(domain)`: `nix.apply`, `wifi.connect`,
  `factory.install`, `blob.fetch`, `firmware.fetch`, `systemctl`,
  `pipewire.volume`, `gpu.power`, `repartition`, `mesh.skill`.

### CLI (`nosctl`)

| Antes | Agora |
|---|---|
| `fetch --confirm` | `fetch --confirm-id <id>` |
| `nix-apply --confirm` | `nix-apply --confirm-id <id>` |
| `wifi-connect --confirm` | `wifi-connect --confirm-id <id>` |
| `transmute --confirm` | `transmute --confirm-id <id>` |
| — | **`nosctl consent <kind>`** (abre a janela + submete, imprime o `confirm_id`) |

`nosctl consent` é o caminho de texto (firstboot/SSH): faz `ui.open` +
`ui.action` e devolve `{"result":{"confirm_id":"…"}}`.

### Firstboot

O Q5 (humano) ou a política `firstbootAuto` da imagem já não mandam
`--confirm`: obtêm o grant com `nosctl consent transmute` e passam
`--confirm-id`. Offline/auto continuam a forçar `--no-download`.

## Evidência

- `ipc_auth::client_hitl_boolean_is_never_consent` — com token admin,
  `transmute.run` + `hitl_confirmed:true` é **negado** no HITL; grant de
  outro kind também.
- `ipc_auth::nix_ring_is_disarmed_by_default` — agora com grant válido, para
  provar que a recusa é `NIX_RING_DISABLED` e não o HITL.
- `hitl_grant_is_single_use_and_kind_bound` mantém-se (replay negado).
- QEMU: boot → firstboot auto → `consent` → `transmute --confirm-id` →
  hotswap → HUD (ver `evidence/HITL_0.6.0-alpha.8.md`).

## Honestidade

- **Isto não torna o grant “prova de humano”.** Um cliente LocalAdmin pode
  chamar `nosctl consent` — o que mudou é que a decisão passou a ser do
  servidor (janela + `ui.action`), single-use e amarrada ao kind, em vez de um
  booleano no payload. Contra um atacante com root local nada disto protege.
- Em modo auto (ISO/lab) o “consentimento” é a política `firstbootAuto`
  gravada na imagem — está documentado, não é chamado de HITL.
- O ring Nix continua desarmado (R-03/R-04).
