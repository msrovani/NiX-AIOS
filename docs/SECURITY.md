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

## HTTP lab / bridge JARBAS

O lab é **off por default**; o ISO liga-o nos modos `cinematic` e `auto`
(`NOSD_ORB_LAB=127.0.0.1:7701`). Quando ligado, o token do processo **nunca**
entra no browser:

- `GET /` devolve o HUD sem qualquer token no HTML/JavaScript.
- O launcher do kiosk gera uma chave bootstrap (0600) e abre
  `http://127.0.0.1:7701/#bootstrap=<chave>`; o fragmento não viaja na rede e
  é substituído por cookie `jarbas_ui` (`HttpOnly`, `SameSite=Strict`, sem
  `Max-Age`) em `POST /ui/session?key=`.
- A chave é **reutilizável** (o kiosk renova a sessão sozinho se o `nosd`
  reiniciar) e substituída a cada arranque do Chromium. Fail-closed: só é
  aceite se o ficheiro for regular, do próprio daemon e não escritível por
  grupo/other — o `data-dir` é 1777 na live ISO, por isso um ficheiro
  plantado ou um symlink é recusado.
- `POST /rpc` exige origem exacta de loopback (nunca por prefixo), cookie
  válido, marcador `X-Jarbas-UI-Request: 1`, `Content-Type: application/json`,
corpo ≤ 1 MiB e timeout de 5 s. Campos `token`/`caps` no JSON do browser são
  recusados (400): a identidade é sempre `AuthContext::JarbasUi` → perfil
  `Jarbas` (HITL/FS/NET/INFER/MEMORY, **sem** `NIX_WRITE`/spawn).
- Allow-list de métodos no kiosk: `health`, `score`, `cognitive.tick`,
  `orb.get|set|sense`, `ui.list`, `ui.action`, `chat`, `voice.turn`.
  `GET /transmute` → 403. Sem `Access-Control-Allow-Origin: *`.
- Sessão com TTL deslizante de 1 h, renovada por cada pedido válido;
  `POST /ui/logout` revoga de imediato.

Não expor o lab à WAN. Em produção, o lab deve ficar desligado
(`NOSD_ORB_LAB=""`), o que elimina esta superfície por completo.

## Confinamento processo

| Camada | nosd | jarbas-cage |
|--------|------|-------------|
| systemd `ProtectSystem` | `strict` | **não** (DRM/seatd) |
| `ReadWritePaths` | dataDir, `/run/nosd`, `/tmp`, `/mnt/nixaios-log` | — |
| Landlock (`NOSD_LANDLOCK=1`) | dataDir + `/var/lib/nosd` + `/tmp` + FAT mount + `/etc/nixos` RO | — |
| User | `nosd` | sessão kiosk |

## HITL (S-04, parcial)

`ui.action` com `Submit` numa janela HITL emite um **grant** server-authored
(`confirm_id`, `confirm_expires_ms`): single-use, kind-bound, TTL 5 min.
`self.heal.evaluate`, `wifi.connect`, `nix.apply`/`confirm`/`build_vm`,
`transmute.run` e `systemctl.scoped` exigem esse grant. Replay, kind errado e
expirado são negados.

**S-04 fechado (0.6.0-alpha.8):** `hitl_confirmed` é **ignorado** no wire. Nem
root/token (LocalAdmin) transforma um booleano do cliente em consentimento —
root já tem `NIX_WRITE`, mas HITL continua a exigir grant. Em texto, o fluxo é
`nosctl consent <kind>` (abre a janela no servidor e submete-a).

## Fetch (S-07)

`fetch_to_dir` aceita apenas basename, cap de 8 GiB, `Content-Length`/stream
cap, `.partial` + rename, magic `GGUF` para `.gguf` e limpeza em falha. Path
traversal (`../`, `/`, `\`) é rejeitado antes da rede.

## Ring Nix (S-06)

`nix.apply`, `nix.confirm`, `nix.build_vm` e `nix.rollback` devolvem
`NIX_RING_DISABLED` por default. O ring root só é armado com
`NOSD_NIX_RING=1`, depois de S-04 (HITL nonce), R-03 (import `ai-managed`) e
R-04 (helper root endurecido) estarem verdes no VM test. `nix.propose` continua
permitido, mas só escreve staging (sem root).

## Blobs e ISO (S1.2)

- ISO **não** embute AgentJev / Piper seed / GGUF S1.
- Runtime: `nos-hermes::blob_deliberate` → Cortex/Hermes → JARBAS grant
  (`confirm_id`, kind `blob.fetch`/`firmware.fetch`).
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
