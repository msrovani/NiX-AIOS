# Desenvolvimento — NixAIOS

## Layout do repo

```
nix-aios/
  flake.nix              # packages nosd, iso, llama-cpp-cuda opt-in
  packages/nosd/         # workspace Cargo
    crates/nosd|nos-* 
  modules/               # NixOS modules
  images/iso.nix         # perfil ISO minimal + firstboot
  scripts/               # build ISO, smokes, metal
  docs/                  # esta documentação
```

## Ferramentas

- Rust 2021, Tokio, Clap, Serde.
- Nix flake (`x86_64-linux`) + `rust-overlay`.
- Testes: `cargo test --workspace` em `packages/nosd`.
- UI JARBAS: feature `nos-jarbas/slint-ui` no flake (`cargoExtraArgs`).

## Ciclo local

```bash
cd packages/nosd
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p nosd resolve_models_dir
cargo test -p nos-mesh crdt
```

Smokes (Linux/WSL):

| Script | Aceite |
|--------|--------|
| `smoke-cognitive-loop.sh` | tokens + remember + recall |
| `smoke-frugal-fleet.sh` | wasmi 32+128 + fuel trap |
| `smoke-cuda-honesty.sh` | ENGINE_ABSENT vs GPU host |
| `smoke-transmute-paths.sh` | Linux-first + Landlock + CRDT |

## Adicionar um RPC

1. Método + caps em `nos-ipc` / CapGate matrix.
2. Handler na fatia `nosd/src/rpc/<domínio>.rs`.
3. Subcomando em `nosctl.rs`.
4. Se side-effect: entrada em `nos-hermes::opar_gate`.
5. Teste host + linha honesty no ledger se fechar item de remigração.

## Firstboot / transmute

- Script Nix: `modules/transmute-stage.nix` (`nos-firstboot`).
- Lógica Rust: `nosd/src/transmute.rs`.
- **Não** preferir FAT sem `NIXAIOS_USE_FAT_PERSIST=1`.

## ISO

```bash
bash scripts/build-iso.sh
bash scripts/guard-iso-no-s1-blob.sh /path/to.iso
```

Política: zero GGUF/AgentJev na imagem.

## Documentação

- Hub: [`docs/README.md`](README.md)
- Ao mudar comportamento user-facing: actualizar GETTING_STARTED / API / CHANGELOG.
- Conventional Commits; ver [`CONTRIBUTING.md`](../CONTRIBUTING.md).
