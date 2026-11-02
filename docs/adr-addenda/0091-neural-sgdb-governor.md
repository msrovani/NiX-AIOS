# ADR-0091 addendum — MemoryGovernor + neural-sgdb in-process

## Status

Accepted (0.7.0-alpha.5 in-tree).

## Decision

1. Cognitive memory = **neural-sgdb** FileStorage behind `nos-memory::NosMemory` / Governor.
2. Hermes/Cortex/RPC **must not** touch `Sgdb`/`FileStorage` (CI grep).
3. Paths: `memory/nsgdb/` ≠ `mesh-crdt/` (fleet CRDT).
4. Default backend `neural`; rollback `NOSD_MEMORY_BACKEND=nsgdb_legacy|jsonl`.
5. Version pin: `docs/NSGDB_VERSION.md` + `scripts/nsgdb-pin-check.sh`.

## Consequences

- Two-pass + null-scope enforced in code.
- Hybrid/semantic honest: demo ≠ cosine.
- Fleet sync = telepathy two-db (F7), never shared FileStorage.
