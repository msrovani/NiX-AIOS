# Evidence — neural-sgdb MemoryGovernor (0.7.0-alpha.5)

## Summary

F0–F7 in-tree: pin + Governor + adapter + IPC/kiosk + writers MOM + embed honesty +
mesh-crdt rename + smoke + **telepathy usável entre peers mesh** (`KIND_MEMORY`).

## Tested

- `cargo test -p nos-memory` (governor, null-scope, SOUL, legacy, neural scoped,
  `telepathy_two_db_exchange`)
- `cargo test -p nos-hermes cognitive_bridge|wrap_opar`
- `scripts/nsgdb-pin-check.sh`
- Kiosk: `memory.recall`/`memory.health` only; `memory.sync` denied

## Telepathy (F7)

- In-process two-db exchange PASS.
- Wire: `memory.sync` + `nosctl memory-sync` + job hook no `NOSD_JOB_BIND`.
- Docs: `docs/FLEET_TELEPATHY.md`.

## Not-tested

- `NOSD_MEMORY_BACKEND=neural` e2e QEMU/metal
- Embedder multilingual iso-hw
- Telepathy metal multi-host (JOB_BIND + Noise em 2+ máquinas)
- Migração one-shot legacy→neural de corpora grandes

## Pin

See `docs/NSGDB_VERSION.md` — rev `ce5a5c70e1cfa50eb73af68f36d856c8a6f2bbf1` (1.5.0).
