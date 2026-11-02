# neural-sgdb pin (NixAIOS)

Fonte canónica do substrato de memória cognitiva.

```text
nsgdb_crate_version: 1.5.0
nsgdb_tag: (untagged; rev pin)
nsgdb_rev: ce5a5c70e1cfa50eb73af68f36d856c8a6f2bbf1
mcp_contract_version: 1.5.0
doctrine_digest: see scripts/nsgdb-pin-check.sh
nixaios_min: 0.7.0-alpha.5
repo: https://github.com/msrovani/neural-sgdb
```

## Bump checklist

1. Ler CHANGELOG / `contract.json` / `docs/doctrine.md` do nsgdb.
2. Actualizar `packages/nosd/Cargo.toml` (git rev) + `cargo update -p neural-sgdb`.
3. Actualizar este ficheiro + seed `packages/nosd/crates/nos-memory/doctrine/DOCTRINE.md`.
4. Diff doutrina → SOUL + MemoryGovernor allowlists.
5. `cargo test -p nos-memory -p nos-hermes` + `bash scripts/nsgdb-pin-check.sh` + `bash scripts/smoke-nsgdb-memory.sh`.
6. Entrada CHANGELOG NixAIOS: `nsgdb … → …` + Tested/Not-tested.
7. Se `contract.json` breaking: bump campo IPC `memory.api_version`.

**Não** seguir `main` flutuante em ISO. Lab: `NOSD_NSGDB_DEV=1` fora de release.
