# API — nosctl / IPC

Transporte: Unix socket (`/run/nosd/nosd.sock`) ou TCP lab (`--tcp 127.0.0.1:7700`).  
Framing: JSON `Request` / `Response` (`nos-ipc`). Caps no request são **ignorados** pelo daemon (I.1).

## CLI (`nosctl`)

```bash
nosctl --socket /run/nosd/nosd.sock <cmd>
nosctl --tcp 127.0.0.1:7700 <cmd>
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

Quando `NOSD_ORB_LAB` (default `127.0.0.1:7701`):

- `GET /` — lab cinemático  
- `GET /transmute` — wizard HTML  
- `POST /rpc` — proxy IPC  

Não expor à WAN.

## EventBus (amostra)

Eventos internos (não API estável externa): `HERMES_S2_DONE`, `MESH_OFFLOAD_HINT`, `SOUL_MIRROR`, `OBSERVE_SNAPSHOT`, …
