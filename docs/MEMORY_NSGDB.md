# Memória cognitiva NixAIOS = neural-sgdb

Ver pin: [`NSGDB_VERSION.md`](NSGDB_VERSION.md). Plano: governança + sync de versão.

## Paths

| Uso | Path |
|-----|------|
| FileStorage cognitivo | `<data-dir>/memory/nsgdb/` |
| Legacy seg.bin (rollback) | `<data-dir>/nsgdb/L{2,3}/` |
| Mesh CRDT (não é memória) | `<data-dir>/mesh-crdt/` (legacy: `nsgdb/crdt/`) |

Env: `NOSD_MEMORY_BACKEND=neural|nsgdb_legacy|jsonl` (default **neural**).

Lab Windows: `mklink /J packages\nosd\vendor\neural-sgdb C:\DEV\neural-sgdb`
(mesmo rev que `NSGDB_VERSION.md`). Flake injecta o input `neural-sgdb` em `vendor/`.

## Gateway

**Só** `nos_memory::governor::MemoryGovernor` (e `NosMemory` que o envolve).  
Proibido `FileStorage` / `Sgdb::` fora de `nos-memory`.

## Scopes AIOS

`aios/user`, `aios/node`, `aios/fleet`, `aios/skills`, `aios/session`, `nsgdb/doctrine`, global.

Entities MOM: `mom/constraint|decision|fact|pattern|anti-pattern|learning|pref`.

## Ops

IPC/CLI/MCP: `memory.recall|remember|health|curate|decide` — caps em nos-ipc; kiosk só read.

## Consultivo ≠ expressão

O neural-sgdb é **busca de evidência** para inferência (Hits tipados).  
**Não** é a voz do JARBAS: doutrina/protocolo não entram como expressão no
turno de chat. Prompt: Persona (expressão) → `## Memory search` (consultivo)
→ User. Doutrina em `nsgdb/doctrine` ensina o *host/agente*; o GGUF não a
recita. Marcadores: `CONSULTATIVE` `NOT_EXPRESSION` `HIT_NOT_ORDER`.

## Cold-start

Open → seed doctrine → health(era) → recall scopes → prefs → chat (two-pass).

## Frota (F7)

Telepathy two-db; nunca dois writers no mesmo ficheiro. Ver [`FLEET_TELEPATHY.md`](FLEET_TELEPATHY.md).

## Inventário writers (F0/F3)

| Writer | Scope | Entities |
|--------|-------|----------|
| chat transcript | `aios/session` | `mom/learning` |
| OPAR | `aios/node` | `mom/pattern` |
| seek/autolearn | `aios/skills` | `mom/pattern` |
| blob deliberate | `aios/node` | `mom/decision` |
| user remember RPC | `aios/user` | `mom/fact`/`mom/pref` |
| doctrine seed | `nsgdb/doctrine` | `doc/protocol` |
