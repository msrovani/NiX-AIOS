# Contribuir para o NixAIOS

Obrigado por ajudar. Este projeto prioriza **honesty** (ADR-0113), **CapGate** e código pequeno/coeso.

## Antes de abrir PR

1. Ler [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) e [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
2. Correr testes:

```bash
cd packages/nosd
cargo test --workspace
```

3. Smokes relevantes (WSL/Linux):

```bash
bash scripts/smoke-cognitive-loop.sh
bash scripts/smoke-transmute-paths.sh
```

4. Se tocar ISO/blobs: `bash scripts/guard-iso-no-s1-blob.sh <artefacto>` — **S1.2 REJECT** (nunca seed AgentJev/GGUF na ISO).

## Convenções

- **Idioma:** docs e mensagens de utilizador em pt-BR quando fizer sentido; código/idents em inglês.
- **Commits:** Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`).
- **Licença:** AGPL-3.0 (workspace Cargo).
- **Ficheiros:** preferir menos de 300 linhas; funções com uma responsabilidade.
- **Honesty:** não marcar VERIFIED sem evidência (teste host ou journal metal). Atualizar [`docs/REMIGRATION_PLAN_LEDGER.md`](docs/REMIGRATION_PLAN_LEDGER.md) quando fechar um item.
- **Não** introduzir `ProtectSystem=strict` no unit `jarbas-cage` (quebra DRM/seatd).
- Persistência default = `/var/lib/nosd`. FAT só com `NIXAIOS_USE_FAT_PERSIST=1`.

## Fluxo sugerido

1. Issue ou nota no ledger descrevendo o gap.
2. Branch curta; um tema por PR.
3. Testes + nota de honesty na descrição do PR.
4. Review humano obrigatório antes de merge em main.

## Onde pedir ajuda

- Arquitectura / ADRs: `docs/adr-addenda/`
- Metal / ISO: `docs/OPERATIONS.md`
- Segurança: `SECURITY.md` / `docs/SECURITY.md`
