# API — nosctl / IPC

Transporte: Unix socket (`/run/nosd/nosd.sock`) ou TCP lab (`--tcp 127.0.0.1:7700`).  
Framing: JSON `Request` / `Response` (`nos-ipc`).

## Autenticação (S-01/S-02/S-03)

A identidade **não** vem do payload. `caps`, `cap_gen` e `agent` são legado e
**ignorados** para autorização.

| Transporte | Identidade | Como usar |
|---|---|---|
| Unix socket | `SO_PEERCRED` (uid/gid) **ou** token | `nosctl --socket /run/nosd/nosd.sock --token-file <path> <cmd>` |
| TCP loopback | token **obrigatório** | `nosctl --tcp … --token-file <path> <cmd>` |
| HTTP lab `/rpc` | token + same-origin + JSON | token injetado na página; sem CORS `*` |

`nosctl` tenta ler o token (explícito, `NOSD_IPC_TOKEN_FILE`, `/run/nosd/ipc.token`,
`./data/ipc.token`). Com token válido → `LocalAdmin`; sem token, cai em
`SO_PEERCRED`. JARBAS não envia token e fica `Jarbas` (sem `NIX_WRITE`).

Perfis derivados do transporte:

- uid `0` ou gid admin → `LocalAdmin` (todos os bits);
- uid do daemon (`nosd`) → `Jarbas` (HITL/FS/NET; **sem** `NIX_WRITE`/spawn);
- outro uid local → `LocalUser` (chat/read-only; sem HITL/NIX_WRITE);
- sem credencial / token inválido → `Unknown` (deny).

O daemon grava o token em `<data-dir>/ipc.token` (modo `0600` em Unix) e nunca
o loga. Para rotacionar (revoga tokens TCP/HTTP antigos):

```bash
nosctl --tcp 127.0.0.1:7700 --token-file ./data/ipc.token security-rotate
```

TCP em endereço não-loopback exige `--ipc-token` explícito.

## CLI (`nosctl`)

```bash
nosctl --socket /run/nosd/nosd.sock <cmd>
nosctl --tcp 127.0.0.1:7700 --token-file ./data/ipc.token <cmd>
```

### Saúde e observação

| Comando | Método IPC | Notas |
|---------|------------|-------|
| `health` | `health` | hermes, mesh, landlock, llama posture |
| `score` | `score` | `NOSD_SCORE` + early_boot se existir |
| `observe` | `observe.snapshot` | journal/NM/sysfs best-effort |

### Modelos

| Comando | Método |
|---------|--------|
| `models` | `model.list` |
| `suggest` | `model.suggest` |
| `plan` | `model.plan` |
| `fetch …` | `model.fetch` |
| `load PATH` | `model.load` |
| `unload` | `model.unload` |

### Chat / memória / rota

| Comando | Método | Caps efectivos (server) |
|---------|--------|-------------------------|
| `chat PROMPT` | `chat` | INFER \| MEMORY |
| `submit` / `poll` | chat async S2 | INFER |
| `recall QUERY` | `memory.recall` | MEMORY |
| `route PROMPT` | `route.explain` | — |
| `tick` | `cognitive.tick` | — |

### Agentes / artefactos

| Comando | Método |
|---------|--------|
| `spawn [--wasm-b64]` | `agent.spawn` |
| `artifact-validate` | validação MD 0052 |

### UI / orb / voz

| Comando | Método |
|---------|--------|
| `ui-list` / `ui-open` / `ui-close` / `ui-action` | `ui.*` |
| `orb-get` / `orb-set` | `orb.*` |
| `voice-status` / `voice-turn` | `voice.*` |

### Nix / Wi‑Fi / transmute

| Comando | Método | Notas |
|---------|--------|-------|
| `nix-propose` | `nix.propose` | staging |
| `nix-apply` | `nix.apply` | NIX_WRITE + helper root |
| `wifi-scan` / `wifi-connect` | `wifi.*` | HITL |
| `transmute-status` | `transmute.status` | |
| `transmute [--confirm] [--no-download] [--locale]` | `transmute.run` | NET\|FS\|NIX_WRITE; HITL obrigatório p/ download |

`nix.propose` é **fail-closed**: sem `nix-instantiate` no PATH devolve
`syntax_ok:false` (não há “structural OK”). `nix.apply`/`confirm`/`build-vm`/
`rollback` devolvem `NIX_RING_DISABLED` até `NOSD_NIX_RING=1`.

Resposta: JSON pretty-print; exit ≠0 se `ok: false`.

## Response shape

```json
{
  "id": 1,
  "ok": true,
  "error": null,
  "result": { }
}
```

`transmute.run` devolve `TransmuteRunResult`: `models_dir`, `downloaded`, `errors`, `hotswap_triggered`, `note`.

## HTTP lab (orb)

Quando `NOSD_ORB_LAB` (default **off**; opt-in `127.0.0.1:7701`; o ISO só liga no
modo cinematic):

- `GET /` — lab cinemático (injeta o token da sessão na página)
- `GET /transmute` — wizard HTML
- `POST /rpc` — proxy IPC autenticado: token obrigatório, `Content-Type: application/json`,
  same-origin quando há `Origin`, corpo ≤ 1 MiB, timeout de 5 s. Sem CORS `*`.

Não expor à WAN. Em produção, o lab deve ficar desligado (`NOSD_ORB_LAB=""`).

## EventBus (amostra)

Eventos internos (não API estável externa): `HERMES_S2_DONE`, `MESH_OFFLOAD_HINT`, `SOUL_MIRROR`, `OBSERVE_SNAPSHOT`, …
