# F7 — Fleet telepathy (two-db) entre peers mesh

Memória cognitiva **não** é o CRDT `mesh-crdt/`. Cada nó tem o seu
`FileStorage` em `<data-dir>/memory/nsgdb/`. Telepathy replica **records**
(NMD1) pelo **mesmo link Noise** dos jobs mesh (`NOSD_JOB_BIND`).

## Regra

- Um writer por ficheiro DB.
- Dois nós = dois DBs + `memory.sync` (exchange bidirecional).
- Mesh CRDT (`NSGC`) = frota (pins/jobs); telepathy = cognitiva.
- Só peers **pinados** (TOFU) — o frame `KIND_MEMORY=4` exige identidade.

## Uso

Pré-requisitos nos dois nós:

```bash
export NOSD_MESH_REQUIRE_SIG=1   # ou chave já existente
export NOSD_JOB_BIND=0.0.0.0:19001
export NOSD_MEMORY_BACKEND=neural   # default
```

No nó A (peer B em `192.168.50.12:19001`):

```bash
nosctl --tcp … memory-sync 192.168.50.12:19001 --max 64
# ou RPC memory.sync { "addr": "…", "max": 64 }
```

Protocolo (1 RTT):

1. A exporta até `max` records L2/L3 (sem doctrine seed).
2. A → B `KIND_MEMORY` `{op,records_b64,max}`.
3. B `merge_remote` + exporta os seus; responde.
4. A importa o bundle de B.

`memory.health` → `telepathy: ready` quando o backend está aberto.

## Caps / kiosk

| Método | Cap | Kiosk |
|--------|-----|-------|
| `memory.sync` | MEMORY | **não** |
| `memory.recall` / `health` | NONE | sim |

## Honesty

- Conflitos causais → `conflict` (não LWW cego).
- Legacy/jsonl: JSON `MemoryRecord` (não NMD1 completo).
- Sem `NOSD_JOB_BIND` / sem identidade → sync falha fechado.

## Ver também

- `docs/MEMORY_NSGDB.md`
- neural-sgdb `examples/telepathy_two_db`
- Evidência: `docs/evidence/NSGDB_MEMORY_GOV_0.7.0-alpha.5.md`
