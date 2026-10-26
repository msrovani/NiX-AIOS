# Segurança — detalhe técnico

Ver também [`../SECURITY.md`](../SECURITY.md) (reporting).

## Princípios

1. **Least privilege** — CapGate por agente; WASM Frugal sem ambient authority.
2. **Server-authored caps (I.1)** — cliente não escolhe bitmask; `0xFFFF_FFFF` ignorado.
3. **Fail-closed** — methods desconhecidos / caps em falta → deny.
4. **HITL para efeitos externos** — fetch de blobs, Wi‑Fi, `nix.apply`, SelfHeal.
5. **Honesty** — EngineAbsent / SoftAllowlist explícitos; sem fingir Verified.

## CapGate

Bits (resumo): `INFER`, `NET`, `FS`, `AGENT_SPAWN`, `MEMORY`, `NIX_WRITE`, `HITL`, `OBSERVE`.

Perfis server-side: `LocalAdmin` (nosctl), `Jarbas`, `HermesInternal`, `WasmSkill`, `Observe`, `Unknown`→zero.

## Confinamento processo

| Camada | nosd | jarbas-cage |
|--------|------|-------------|
| systemd `ProtectSystem` | `strict` | **não** (DRM/seatd) |
| `ReadWritePaths` | dataDir, `/run/nosd`, `/tmp`, `/mnt/nixaios-log` | — |
| Landlock (`NOSD_LANDLOCK=1`) | dataDir + `/var/lib/nosd` + `/tmp` + FAT mount + `/etc/nixos` RO | — |
| User | `nosd` | sessão kiosk |

## Blobs e ISO (S1.2)

- ISO **não** embute AgentJev / Piper seed / GGUF S1.
- Runtime: `nos-hermes::blob_deliberate` → Cortex/Hermes → JARBAS `--confirm`.
- Guard de build: `scripts/guard-iso-no-s1-blob.sh`.

## Rede e fetch

- HF/HTTPS via `ureq` com CapGate `NET|FS`.
- Seek-before-generate: mesh/GH/GL antes de synth WASM.
- Mesh UDP: TOFU settle; CRDT thin local (sync P2P pleno residual).

## Nix ring

- `nix.propose` staging sob CapGate.
- `nix.apply` via unit root `nosd-nix-apply` — **não** como user nosd.
- ISO prod: `autoSwitch=false`, `requireHitlConfirm=true`.

## SelfHeal (0.5.0 stub)

`evaluate_self_heal`: Deny-by-default sem HITL + `NIX_WRITE`. Não executa firmware sozinho.

## Superfície de ataque (checklist)

- [ ] Socket Unix `/run/nosd/nosd.sock` — permissões de runtime.
- [ ] TCP lab `7700` — só localhost em bancada.
- [ ] Orb lab `7701` — lab; não expor à WAN.
- [ ] Modelos em `/var/lib/nosd/models` — ownership `nosd`.
- [ ] Log FAT legado — umask/uid; preferir Linux FS 0.5.0.
