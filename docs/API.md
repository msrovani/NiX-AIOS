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
| HTTP lab `/rpc` | cookie `jarbas_ui` + origem exacta + marcador `X-Jarbas-UI-Request` | chave bootstrap → `POST /ui/session`; sem token no payload, sem CORS `*` |

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

## HITL grant (0.5.5)

Uma janela HITL (`ui.open`/`ui.card`) com `grant_kind` (ex.: `self.heal`,
`wifi.connect`, `nix.apply`, `transmute`) devolve, no `ui.action` Submit:

```json
{ "confirm_id": "…", "confirm_expires_ms": 123..., "grant_kind": "self.heal" }
```

A ação destrutiva correspondente deve enviar `confirm_id` no `params`. O grant é
single-use, kind-bound e expira em 5 min; replay/kind errado = deny.
**`hitl_confirmed` é ignorado** (S-04 fechado): nem root/token transforma um
booleano do cliente em consentimento. Em texto: `nosctl consent <kind>`.

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
| `doctor [--json]` | agrega 6 leituras | exit 1 se algum subsistema falhar (SKIP≠OK) |
| `config-show` | `config.show` | configuração efectiva; env `NOSD_*` **sem segredos** |
| `energy` | `observe.energy` | joules/W por janela + `source` (rapl/nvml/tdp_estimate/absent) |
| `trace [--last N]` | `cognitive.trace` | OPAR: domínio + comprimentos; **sem notas/PII** |
| `dag-plan FILE` | `dag.plan` | valida DAG + write-scope; **não executa** |
| `dash [--plain]` | health+score+energy+pending | HUD de terminal (serial/metal), zero deps |

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
| `pending` | `ui.pending` (fila HITL: tier + TTL; **nunca** o nonce) |
| `orb-get` / `orb-set` | `orb.*` |
| `voice-status` / `voice-turn` | `voice.*` | `voice.stt` aceita `{vad:true}` (escuta até silêncio) |
| `health` | `health` | inclui `memory_records` (fragmentos nsgdb) |

### Skills / MCP

| Comando | Método | Notas |
|---------|--------|-------|
| `skill-install NAME PATH [--caps log,aios_ai.infer] [--profile app]` | `skill.install` | wasm/wat, hash BLAKE3, imports em allowlist, caps declaradas, perfil `app` exige `NOSD_SKILL_APP_PROFILE=1` |
| `skill-list` | `skill.list` | junta manifesto: versão, foco, caps, `kind`, ABI, proveniência |
| `skill-run NAME --arg N` | `skill.run` | `fuel_used`, `output`, `result`/`result_b64` (ABI 2), `caps_declared` vs `caps_granted` |
| `skill-import FILE.md` | `skill.import` | `SKILL.md` **declarativo**; caps em allowlist; não activa |
| `mcp-list` | `mcp.list` | tools filtradas pelas caps do peer |
| `mcp-call TOOL --args JSON` | `mcp.call` | caps **da tool** no `CapGate`; side-effect exige HITL |

As caps declaradas numa skill **não** são concedidas automaticamente: em
execução valem `caps declaradas ∩ NOSD_SKILL_GRANT_CAPS` (default `log`;
`aios_ai.infer` exige opt-in do operador). Imports fora de
`aios_ai.infer`/`aios_log.write` são recusados no install.

`skill.run` está sujeito a **quota por peer autenticado** (janela
deslizante, persistida em `<data-dir>/skill_quota.json`):
`NOSD_SKILL_QUOTA_RUNS` (240), `NOSD_SKILL_QUOTA_FUEL` (5 000 000) e
`NOSD_SKILL_QUOTA_WINDOW_SECS` (60). O peer é
`unix:<uid>`/`tcp:token:<geração>`/`jarbas-ui`, nunca um campo do payload. A
run é reservada antes de ler o módulo e o fuel medido é cobrado depois; o
excedente devolve `ok:false` com `quota` no erro. A janela sobrevive a um
restart do `nosd` (um ficheiro ilegível recusa globalmente até ser removido).
Com identidade M6 (`NOSD_MESH_KEY`/`NOSD_MESH_REQUIRE_SIG=1`) a janela é
assinada Ed25519 e tem âncora anti-apagamento/anti-rollback: editar, apagar
só a janela, assinar com outra chave ou repor um ficheiro antigo recusa em
vez de repor orçamento. Sem identidade, o arranque registra
`skill quota signed=false` e o ficheiro não é assinado. Em NixOS estes três
valores são `services.nosd.skillQuota{Runs,Fuel,WindowSecs}`.

`mcp.call` tem piso IPC `NONE` de propósito: o registry é server-authored e a
autorização é por tool (`nos-mcp`). O transporte **stdio** é servido pelo bin
`nosmcp` (read-only: `nsd.health|score|energy|pending|trace`); HTTP/SSE e
`resources/*`/`prompts/*` **não** implementados.

### MCP por stdio (`nosmcp`)

```bash
nosmcp [--socket /run/nosd/nosd.sock] [--tcp 127.0.0.1:7700 --token-file …]
```

Fala JSON-RPC 2.0 linha a linha em stdin/stdout (`initialize`, `tools/list`,
`tools/call`, `ping`; `-32700` com `id:null` em JSON inválido). **Não** expõe
escrita — para agir usa-se `nosctl` (com grant HITL onde aplicável).

### Nix / Wi‑Fi / transmute

| Comando | Método | Notas |
|---------|--------|-------|
| `nix-propose` | `nix.propose` | staging |
| `consent <kind>` | `ui.open`+`ui.action` | emite `confirm_id` (server-authored) |
| `nix-apply --confirm-id` | `nix.apply` | NIX_WRITE + helper root; ring desarmado |
| `wifi-scan` / `wifi-connect --confirm-id` | `wifi.*` | HITL por grant |
| `transmute-status` | `transmute.status` | |
| `transmute --confirm-id [--no-download] [--locale]` | `transmute.run` | NET\|FS\|NIX_WRITE; HITL por grant |

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

Quando `NOSD_ORB_LAB` (default **off**; opt-in `127.0.0.1:7701`; a ISO liga-o no
modo `cinematic`/`auto`):

- `GET /` — HUD JARBAS (sem token incorporado no HTML)
- `POST /ui/session?key=<bootstrap>` — troca a chave do launcher (0600, gerada
  também em bancada no arranque do lab) por cookie `HttpOnly`;
  `http://127.0.0.1:7701/#bootstrap=<chave>` para uso manual
- `POST /ui/logout` — revoga a sessão
- `POST /rpc` — proxy IPC da UI: origem exacta de loopback, cookie,
  `X-Jarbas-UI-Request: 1`, `Content-Type: application/json`, corpo ≤ 1 MiB,
  timeout 5 s e **allow-list de métodos** (`health`, `score`,
  `cognitive.tick`, `cognitive.trace`, `orb.*`, `ui.list`, `ui.action`,
  `ui.pending`, `observe.energy`, `chat`, `voice.turn`). Campos
  `token`/`caps` são recusados; identidade sempre `JarbasUi`.
  `health.mesh_nodes` é a topologia read-only usada pelo organograma
  (`MeshEngine::topology`); não abre método novo.
- `GET /transmute` — 403 (o wizard fica nos canais de operador)

Não expor à WAN. Em produção, o lab deve ficar desligado (`NOSD_ORB_LAB=""`).

## EventBus (amostra)

Eventos internos (não API estável externa): `HERMES_S2_DONE`, `MESH_OFFLOAD_HINT`, `SOUL_MIRROR`, `OBSERVE_SNAPSHOT`, …
