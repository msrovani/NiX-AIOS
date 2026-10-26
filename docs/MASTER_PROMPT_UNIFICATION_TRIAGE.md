# Triagem — Prompt Mestre Unificação NixAIOS (2026-10-25)

**Plano de implementação:** [`MASTER_PROMPT_UNIFICATION_PLAN.md`](MASTER_PROMPT_UNIFICATION_PLAN.md)  
**Regra:** não reexecutar ondas inteiras já no ledger; só gaps viáveis. Fonte: `REMIGRATION_PLAN_LEDGER.md` + ADR-0113.

## Mapa rápido (prompt → realidade)

| Secção do prompt | Ledger | Acção nesta onda |
|------------------|--------|------------------|
| nosd-early + BOOT SCORE pós-sysinit | B.1 PARTIAL | **Harden** early: observe+score+JSON, Before=getty |
| System-1 AgentJev mmap &lt;100ms ISO | S1.2 **REJECT** | Honesty: keyword S1; blob só runtime LOO |
| nos-observe → EventBus | NX.3 DONE host | Early chama `observe.snapshot`; DBus link = residual |
| CapGate server-authored | I.1 DONE | Sem mudança |
| Landlock nosd + cage | I.3 PARTIAL | Cage: ProtectHome+DeviceAllow+OOM=-500 (ProtectSystem=strict **REJECT** — quebra seatd/DRM); Landlock syscall = residual |
| nosd-engine@ cgroup ≠ cage | I.5 PARTIAL↑ | MemoryMax + OOMScoreAdjust=+200 no engine; cage OOM=-500 |
| WASM Frugal 256KiB | F.1/P.8 DONE | — |
| NX.0–NX.6 / F.3 / FederatedIndex | DONE host | — |
| Orb 3 Z-order + LatentBus f16[256] | P.2/J.2 PARTIAL | **DEFER** (ADAPT Slint ≠ bare compositor) |
| CRDT mesh / nsgdb L5–L7 | M.2/DB.1 residual | **DEFER** |
| Clone/compile neural-os no boot | — | **REJECT** (já no flake ISO) |

## Early boot honesty

```
sysinit → nosd.service → nosd-early-score
  → nosctl observe  (OBSERVE_SNAPSHOT)
  → nosctl score    (BOOT SCORE → journal + /var/lib/nosd/boot-score.json)
  → S1 backend = keyword (AgentJev ABSENT até fetch HITL)
```

IA “no early boot” = **observe + score + Hermes Continuous**, não LLM 3B no sysinit.

## Não fazer nesta onda

Reescrever flake do zero; Orb DRM nativo; AgentJev embutido na ISO; CRDT pleno; build-vm e2e metal.
