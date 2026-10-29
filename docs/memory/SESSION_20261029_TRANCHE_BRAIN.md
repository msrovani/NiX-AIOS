# Sessão 2026-10-29 — Tranche G0–G5 + Brain self-upgrade + saga nixaios-ring

**Sessão audit/tranche:** auditoria profunda docs↔código → implementação do plano
aprovado (G0→G5) → brain self-upgrade HITL completo → check `nixaios-ring`
(stub rebuild) a apanhar 3 bugs latentes reais.

Commits: `d64db33` (G0/G1/G2/G3/G5.1) · `d647ed8` (checks VM/ring + CLI brain UI)
· `71538ad` (Qwen3.5-0.8B como derivação Nix) · `54a72xx` → `1cf9d0b`
(modulos/nosd-brain) · `05418d3` (gnugrep latente) · `e4a03cc` (split pretty JSON)
· `7551fd1` (brain self-upgrade completo) · `8681e09` (sessão HW: Qwen no
first boot iso-hw + fix escaping; transporta à força meu fix daemon-NIX_RING).

## 1. O que ficou provado (host-level)

- `cargo test --workspace` verde (~300 testes; +20 novos desta tranche).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` zero.
- `nix-instantiate --parse` de 7 módulos OK (loop `_tmp-nix-parse-check.sh`
  activo; lição 26).
- `nixaios-ring` (novo): helper root COM ring ARMADO ponta-a-ponta com
  `nixos-rebuild` stub — o alvo é o **protocolo** do helper (fases/TTL/negativos),
  não o rebuild real (sem nixpkgs resolúvel dentro de VM tester).

## 2. Os 3 bugs latentes que o `nixaios-ring` apanhou (saga)

| R# | Sintoma no VM | Bug real | Fix |
|---|---|---|---|
| 1 | `grep: command not found`, exit 127 | helper ARMADO usa `grep/cut` e o `path` do unit não tinha `pkgs.gnugrep` — nunca visto porque os testes antigos só corriam o caminho desarmado (sai antes do grep) | gnugrep no path do unit + do stub `mkForce` (`05418d3`) |
| 2 | `IndexError` no split do `confirm_id` | `serde_json::to_string_pretty` imprime `"key": "value"` com espaço — split exato quebra | split por `'"confirm_id":'` + índice de aspas (`e4a03cc`) |
| 3 | daemon recusa `NIX_RING_DISABLED` com ring "armado" | `armRing=true` só armava o HELPER; o kill switch do DAEMON (S-06) continuava desarmado | `++ lib.optional cfg.armRing "NOSD_NIX_RING=1"` no Environment do unit nosd (em HEAD via `8681e09`) |

Lição-mestra: **bug de PATH/systemd não se prova com unit test** — e o
oposto também vale: o caminho ARMADO nunca tinha test ponta-a-ponta; o
desarmado não cobre os erros do armado (lições 23/24 do AGENTS.md).

## 3. Brain self-upgrade (o ciclo pedido)

- `brain.plan` (Observe, caps NONE): posture OBSERVADO → tier alcançável:
  **T2** Qwen3.5-0.8B **Q8_0** (mesma rede, Q4→Q8) · **T3** **4B Q4_K_M**
  (5x, CPU/iGPU) · **T4** **9B Q4_K_M** (alvo dGPU supervisionada; **exige
  ring ARMADO**). Custos exatos do pin (download/disco MB), RAM/VRAM "est."
  rotulado; ganho qualitativo honesto (p95 medido depois, não prometido);
  sha256 pinado impresso na card; rollback = `nosctl load <anterior>`.
- `brain.accept` (Escalate — grant `brain.upgrade` server-authored,
  single-use): fetch → **verificação sha256** (diverge = apagado,
  fail-closed) → hot_load → hub register → OPAR.
- Gancho pós-boot: `NOSD_BRAIN_PROPOSE=1` → card HUD a cada 30 ticks com
  anti-spam 24h no CRDT (`brain/upgrade/last`, replicável pela frota).
- REGISTO de tiers com sha256/bytes dos **LFS oids** do HF tree
  (unsloth/…GGUF, Apache-2.0, base Qwen/Qwen3.5-0.8B).
- Contratos: caps nos-ipc (`brain.plan` NONE; `brain.accept` NET|FS|HITL),
  side-effect OPAR ("brain.accept"), `brain_plan_and_accept_caps`.
- Cli: `nosctl brain-plan` / `brain-accept <tier> --confirm-id` /
  `consent brain.upgrade`.

## 4. Qwen3.5-0.8B — os dois caminhos (sessão paralela OK)

- **Sessão HW (8681e09):** download runtime no first boot da iso-hw
  (llama.cpp incluído; `NOSD_BOOT_MODEL_*`); **provado carregar no host`
  (evidence QWEN35_0.8B_0.6.0-alpha.12); /v1/chat/completions residual.
- **Tranche presente:** `packages.qwen35-08b-q4km`/`qwen35-08b-q8`
  (derivação content-addressed, pin SRI) + `services.nosdBrain` (symlink L+
  em models) — **upgrade** da iso-hw para "sem download no primeiro boot".
- **Caminho canónico do upgrade pós-boot:** brain.plan propose → HITL →
  tier fetch (sha pin) → hot_load. ADR addenda
  `adr-addenda/0118-qwen-embedded-brain.md`.

## 5. Residual honesto (o que não foi provado)

1. **Sem OBSERVED nesta tranche**: VM re-prova (`nixaios-vm` com âncora
   assinada + pins/seq + ring) em build quando este ficheiro foi escrito;
   Q-03 (CI externo) pendente de push; metal dGPU = AWAITING_HW.
2. Fetch real de 812 MB/2,5 GB com hot_load do tier — só observável na VM
   com rede/hardware; o check usa fixtures pequenas.
3. Âncora assinada fecha a FORJA; overwrite pelo dono do data-dir continua
   admitido (remédio: ancora fora do data-dir).
4. `services.nosd.package` ainda não chega a `transmute.rs` (sessão HW
   mexeu lá). Lucro do brain-upgrade auto-detectado em frota (peer sync do
   posture) ainda não.
5. `curve p95> para tier2/3 medido só depois do primeiro hot_load em VM/metal.
