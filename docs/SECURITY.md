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

A identidade é resolvida pelo **transporte** (S-01), não pelo payload:

- **Token-first** em qualquer transporte: se o pedido traz o token de
  `<data-dir>/ipc.token` (modo `0600`), é `LocalAdmin`; token inválido → `Unknown`
  (deny, sem downgrade silencioso).
- Sem token, Unix usa `SO_PEERCRED`: uid `0`/gid admin → `LocalAdmin`; uid do
  daemon → `Jarbas`; outro uid → `LocalUser`; sem credencial → `Unknown`.
- TCP/HTTP **exigem** token. `security.rotate` troca token + geração e revoga
  tokens antigos.
- `caps`, `cap_gen` e `agent` no JSON são legado e **não** elevam permissão.
- JARBAS não envia token (fica `Jarbas`); `nosctl` lê o ficheiro e fica
  `LocalAdmin`. No ISO, o user `nixos` usa `sudo nosctl`/`nos-transmute`.

`LocalUser` tem `INFER|MEMORY|OBSERVE`; `Jarbas` tem `HITL|FS|NET` mas **não**
`NIX_WRITE`/`AGENT_SPAWN`; só `LocalAdmin` tem `NIX_WRITE`.

## HTTP lab

O lab é **off por default**; o ISO só o liga no modo cinematic
(`NOSD_ORB_LAB=127.0.0.1:7701`). Quando ligado, `POST /rpc` exige token,
`Content-Type: application/json`, same-origin quando há `Origin`, corpo ≤ 1 MiB e
timeout de 5 s. Sem `Access-Control-Allow-Origin: *`. A página `GET /` injeta o
token da sessão; não expor o lab à WAN.

## Confinamento processo

| Camada | nosd | jarbas-cage |
|--------|------|-------------|
| systemd `ProtectSystem` | `strict` | **não** (DRM/seatd) |
| `ReadWritePaths` | dataDir, `/run/nosd`, `/tmp`, `/mnt/nixaios-log` | — |
| Landlock (`NOSD_LANDLOCK=1`) | dataDir + `/var/lib/nosd` + `/tmp` + FAT mount + `/etc/nixos` RO | — |
| User | `nosd` | sessão kiosk |

## Ring Nix (S-06)

`nix.apply`, `nix.confirm`, `nix.build_vm` e `nix.rollback` devolvem
`NIX_RING_DISABLED` por default. O ring root só é armado com
`NOSD_NIX_RING=1`, depois de S-04 (HITL nonce), R-03 (import `ai-managed`) e
R-04 (helper root endurecido) estarem verdes no VM test. `nix.propose` continua
permitido, mas só escreve staging (sem root).

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
