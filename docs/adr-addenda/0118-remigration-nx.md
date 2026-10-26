# ADR addendum — NIXAIOS remigration (NX.0 + ledger)

Governance **G.1** — política Nix honesta enquanto bare-metal permanece canônico (ADR-0113).

**Ledger do plano (55 itens DONE/PARTIAL/NOT_DONE):** [`../REMIGRATION_PLAN_LEDGER.md`](../REMIGRATION_PLAN_LEDGER.md) — SESSION_529.

## NX.0 prod defaults

| Knob | Default | Nota |
|------|---------|------|
| `services.nosdNixRing.autoSwitch` | `false` | `NOSD_NIX_AUTO_SWITCH` só em lab quando explícito |
| `services.nosdNixRing.requireHitlConfirm` | `true` | `NOSD_NIX_REQUIRE_HITL=1` |
| `services.nosdNixRing.buildVmFirst` | `false` | opt-in: `build-vm` antes de `switch` |

ISO prod (`images/iso.nix`) segue estes defaults; scripts de lab podem override `autoSwitch = true`.

## Seek-Before-Generate

Propostas Nix (`CapGate NIX_WRITE`) exigem **seek** (plan/search) antes de gerar fragmento — não inventar attrs fora do allowlist em `ai-managed.nix`.

## CapGate server-authored

O daemon **nosd** não executa `nixos-rebuild`; só escreve `nix-apply.request`. Apply e rollback são **root oneshots** (`nosd-nix-apply`, `nosd-nix-rollback`). Evidência de geração: `/var/lib/nosd/last-generation.txt`.

## UI vs compute GPU

- **iGPU:** `jarbas-cage` (cage + Slint/Chromium).
- **dGPU:** `nosd-engine@` (llama-server, CUDA/`NVIDIA_VISIBLE_DEVICES`).

## Rollback

```bash
touch /var/lib/nosd/nix-rollback.request
# path unit → nosd-nix-rollback.service → nixos-rebuild --rollback
```

Ou: `nosd-nix-rollback-request` (helper no path do anel Nix).
