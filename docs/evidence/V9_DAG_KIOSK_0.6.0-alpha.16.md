# V9 — ataque (DAG + kiosk) — 0.6.0-alpha.16

**Data:** 2026-10-29 · Âmbito: V9.1, V9.4, V9.5
([`VAMPIRE_PLAN_OPENJARVIS.md`](../VAMPIRE_PLAN_OPENJARVIS.md) §13).
V9.2/V9.3 ficam com desenho, sem código.

## 1. Veredicto honesto (ADR-0113)

| ID | Entrega | claim_status | Prova |
|----|---------|--------------|-------|
| **V9.1** | DAG com isolamento de write-scope (`nos-core::dag` + `dag.plan` + `nosctl dag-plan`) | **OBSERVED + unit** | 11 testes em `dag`; e2e: diamante → 3 camadas, `max_parallel=2`; conflito recusado |
| **V9.4** | kiosk: `observe.energy` + `ui.pending` na allowlist | **VERIFIED (unit)** | `kiosk_methods_are_scope_limited` (caps NONE/INFER/HITL; 17 métodos proibidos) |
| **V9.5** | `Tested:` / `Not-tested:` nos commits | convenção (lição 36) | este commit |
| V9.2 | memória `prepared` vs `approved` | **desenho** | §3 |
| V9.3 | skills declarativas executáveis | **desenho** | §3 |

## 2. Comandos

```bash
cargo test --workspace        # 374 testes, 0 failed
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all

nosctl dag-plan dag-ok.json       # layers [[root],[l,r],[join]], max_parallel=2
nosctl dag-plan dag-conflict.json # ok:false "camada 0: a e b escrevem em src/rpc"
bash scripts/smoke-nosmcp.sh      # PASS
bash scripts/smoke-vampire.sh     # PASS
```

**Não executado:** ISO/QEMU nesta tranche (a ISO em gravação pelo dono é a
alpha.15; V9 não está nela).

**Linux (`nix build .#nosd`) — PASS:**

```
/nix/store/991w7j7ixhxdf700cd6clls8r4bfahb1-nosd-0.6.0-alpha.16
```

(374 testes correm na derivação; a 1.ª tentativa falhou porque os ficheiros
novos ainda não estavam no índice git — lição 1: o flake lê a árvore git.)

## 3. Desenho dos adiados

### V9.2 — memória `prepared` vs `approved`

- `MemoryRecord` ganha `tier: prepared|approved` (default `approved` para não
  quebrar replay).
- `recall` filtra `prepared` salvo `scope=session/pending` (o turno que a criou).
- Promoção explícita (`memory.approve`) com OPAR; nada é promovido em silêncio.
- Exige `seg.bin` v4 + migração one-shot (registo antigo = `approved`).
- **Porquê esperar:** mexe no M8/CRDT e na âncora assinada; não antes do metal.

### V9.3 — skills declarativas executáveis

- O corpo do `SKILL.md` passa a ser guardado em
  `<data>/skills/<nome>/SKILL.md` (hash BLAKE3 no manifesto; **nunca** no CRDT).
- `skill.render <nome>` devolve o corpo **se** as caps declaradas estiverem
  dentro da allowlist e o publisher (se houver) estiver válido.
- O corpo é injectado no prompt (fragmento), nunca executado: sem wasm, sem
  `sh`, sem FS.
- **Porquê esperar:** precisa de decidir se um fragmento de prompt conta como
  "cap declarada" (aios_ai.infer) e como aparece no score S1/S2.

## 4. Ficheiros

Novos: `nos-core/src/dag.rs` · `nosd/src/rpc/dag.rs` · este ficheiro.
Alterados: `nos-core/src/lib.rs` · `nos-ipc/src/lib.rs` · `nosd/src/nosctl.rs` ·
`nosd/src/rpc/{mod,http_lab}.rs` · `docs/{API,VAMPIRE_PLAN_OPENJARVIS}.md` ·
`CHANGELOG.md` · `AGENTS.md` · `README.md` · `docs/*` (versão).

## 5. Residuais

1. DAG **não executa** nada: só valida. Ligar a `job.submit`/factory é V10.
2. Kiosk: expostos dois métodos read-only; o browser (Chromium cinematic) ainda
   não os usa.
3. Metal continua AWAITING_HW; a ISO em gravação pelo dono é a alpha.15.
