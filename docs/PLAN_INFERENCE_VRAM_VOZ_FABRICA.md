# Plano de correção — inferência, VRAM, voz e fábrica

**Estado:** PLAN (nada executado). **Origem:** revisão de código de 2026-11-06
(21 achados, `file:line` no relatório da sessão). **Norte:** o motor deixa de ser
configurado por heurística nossa e passa a ser decidido pelo `--fit on` do
llama.cpp, com a VRAM **medida**, não somada nem adivinhada.

> **Correcção à revisão (na escrita deste plano).** Duas afirmações do relatório
> ficaram provadas erradas e são substituídas aqui: (1) o conselho térmico
> **tem** consumidor — `main.rs:689-706` baixa `NOSD_LLAMA_NGL` quando há calor
> ou presença `drowsy/sleep`; o defeito é outro e pior (ver C3.3); (2) não se
> pode derivar `block_count` do header como escrito — `parse_gguf_header`
> (`nos-core/src/gguf.rs:23`) só lê magic/versão/`tensor_count`/`kv_count`,
> o número de camadas vive na secção KV (ver "Não entra").

Regras deste plano: uma fase = um commit (AGENTS regra 10); cada tarefa declara
a prova; tudo o que não for observado fica `Not-tested` no corpo do commit
(regra 36, ADR-0113).

## Princípios de desenho

1. **`--fit` decide as camadas.** Não derivamos `-ngl` de estimativas. Um
   `-ngl` explícito é uma escolha do operador, não do daemon.
2. **VRAM e RAM nunca se somam.** Um modelo cabe numa placa ou faz spill
   declarado; "pool" é uma ficção que transforma todo o `TooTight` em `Fits`.
3. **Um modelo, uma cópia.** Se a lane S2 já serve o 4B num servidor externo,
   o daemon não carrega uma segunda cópia no backend gerido.
4. **Nix constrói, o PATH não.** A fábrica já gera `flake.nix`; quem o ignora é
   o runner.
5. **Honestidade.** `loaded`, `evict` e `%` só dizem o que medem. Um conselho
   térmico que ninguém lê é um log, não um controlo.

---

## Fase 1 — A VRAM deixa de ser sobre-subscrita (P0)

É a única fase que muda o arranque em metal. **Primeira, e sozinha no commit.**

### C1.1 — Remover o `-ngl` derivado de heurísticas (três sítios)

- **Achado A (boot):** `packages/nosd/crates/nosd/src/main.rs:245-246` faz
  `set_var("NOSD_LLAMA_NGL", suggest_ngl)` a partir de
  `plan_n_gpu_layers(32, 80, vram_free, 256, 128, 256)`.
- **Achado B (runtime):** `main.rs:689-706` baixa a **mesma** variável quando
  `suggest_ngl_cap` desce por calor (85 °C → 16) ou presença `drowsy/sleep`
  (32/0), com `cur` a ler o env e a default **99**.
- **Achado C (terceiro sítio, escapou à revisão):** o handler
  `observe.snapshot` (`crates/nosd/src/rpc/health.rs:289-298`) repete o mesmo
  `set_var` — quem abre o painel de observação no HUD reescrevia o env.
- **Porque é P0 nos dois:** `hotswap/llama_server.rs` (`ngl_from_env`) e
  `llama_cli.rs` leem a variável e passam `-ngl`, o que **anula a escolha de
  camadas do `--fit`** — a classe de OOM do metal de 3 Nov. No caso B há dois
  agravantes: (i) mudar o env **não afecta** o llama-server que já está a correr
  (a flag foi decidida no spawn), logo não protege nada do calor presente;
  (ii) o valor só desce e **nunca é reposto** (`cap < cur`, e `cur` lê o que
  acabámos de escrever) — depois de um pico térmico, o próximo spawn leva
  `-ngl 16` para sempre.
- **Mudança:** apagar os **três** `set_var`. Boot: log `suggest_ngl=N (hint;
  fit decide)`. Runtime: um `thermal_ngl_cap: AtomicU32` em `nos-cortex`
  (`u32::MAX` = sem cap), escrito pelo tick e pelo `observe.snapshot`, e
  consumido no spawn (C3.3). `NOSD_LLAMA_NGL` continua aceite se o operador a
  definir no unit.
- **Ficheiros:** `crates/nosd/src/main.rs`; comentário no `modules/hw-cuda*.nix`
  a apontar que uma `-ngl` explícita desliga o fit.
- **Aceite:** journal do arranque sem `-ngl` no argv do motor; log do `nosd` com
  `suggest_ngl` como hint. Nenhum teste unitário prova isto (PATH/systemd,
  AGENTS regra 9): a prova é QEMU.
- **Prova:** `gate-boot-to-jarbas --stage MQEMU` + `journalctl -u nosd -b |
  grep -c "ngl="` no spawn. Metal: `nvidia-smi --query-compute-apps` mostra o
  servidor com VRAM e o log do engine mostra `offloaded N/M layers`.
- **Risco R1:** um llama.cpp sem `--fit` passa a correr em CPU (0 camadas). O pin
  actual tem `--fit` (`llama-cpp` 0.6+/11429). Mitigação documentada: `-ngl`
  explícito no unit `hw-cuda` se o binário mudar.

### C1.2 — Não carregar o 4B duas vezes

- **Achado:** `nos-hermes/src/lib.rs:586` (`chat_submit`; idem `:475` em `chat`)
  chama `plan_slot()` (`:652`) **antes** de
  decidir a lane; `plan_slot` → `ensure_loaded_fit` → `ensure_model_loaded` →
  `hot_load_llama` (mata o S1 e spawna o 4B no backend gerido), e só depois o
  ramo S2 usa o `nosd-engine-s2` que **já tem** esse 4B. Duas cópias em 6 GiB.
- **Mudança:** extrair `fn should_hot_load(decision: &RouteDecision, has_s2: bool) -> bool`
  (pura): `false` quando a lane é S2/PRO e existe `hotswap_s2` com esse modelo
  carregado. `chat_submit`/`chat` só chamam `plan_slot` quando devolve `true`.
- **Ficheiros:** `crates/nos-hermes/src/lib.rs` (+ testes na mesma crate).
- **Aceite:** pedido PRO não produz `hotswap hot_load` no log nem reinicia o S1;
  `OpAR` mostra `lane=s2 slot=GeneratorPro` sem `swapped=true`.
- **Prova:** teste unitário da função pura (tabela: lane×s2×modelo) + host com
  duas instâncias (S1 gerido + S2 attach a um `llama-server` local) e
  `ps`/`journal` sem segundo spawn. Metal: `nvidia-smi` com **uma** cópia do 4B.

### C1.3 — `ensure_model_loaded` deixa de devolver um booleano mudo

- **Mudança:** `enum LoadOutcome { Reused, Swapped }`; o log e o `OpAR` passam a
  distinguir "já estava" de "substituído".
- **Ficheiros:** `crates/nos-cortex/src/hotswap/mod.rs`, chamadores (`hermes`,
  `transmute.rs:448`, `rpc/model.rs:309`, `rpc/brain.rs:63`).
- **Aceite/prova:** testes existentes adaptados; zero mudança de comportamento
  no QEMU (é telemetria).

### C1.4 — Health do spawn honesto

- **Achado:** `llama_server.rs::spawn` tem prazo fixo de 30 s; um 4B Q6 a frio
  pode excedê-lo → `EngineAbsent("health timeout")` → fallback silencioso para
  `llama-cli` (com `n_predict` 64).
- **Mudança:** `NOSD_LLAMA_HEALTH_TIMEOUT_SECS` (default 120); em timeout
  **não** cair para CLI — devolver `SwapError::EngineAbsent("llama-server boot
  timeout")` com o tempo gasto, e deixar o chamador decidir (S1 cai para
  cpu-echo, não para um motor pior sem avisar).
- **Prova:** host com `llama-server` artificialmente lento (sleep no wrapper) e
  log com o código correcto; QEMU com o 4B real.

---

## Fase 2 — Fit e hub que dizem a verdade (P1)

Depende de C1.3 (o evict precisa de um motor para descarregar).

### C2.1 — Separar VRAM de RAM no veredicto

- **Achado:** `fit_policy.rs:23` — `pool = avail_ram + avail_vram`.
- **Mudança:** `fit_model(need_vram_mb, need_ram_mb, avail_ram_mb, avail_vram_mb,
  allow_spill: bool)`; `Fits` exige caber na VRAM, ou caber no spill declarado
  (S1/CPU sim, S2/CUDA não). `FitReport` ganha `basis` (`vram|spill|cpu`) para
  não voltar a haver percentagens sem denominador.
- **Call sites (3):** `hub.rs:86`, `airllm.rs:47`, `blob_deliberate.rs:57`.
- **Prova:** testes de tabela (4B Q6 em 5,5 GiB + 12 GiB RAM = não `Fits`);
  `model.plan`/`blob.deliberate` com o mesmo JSON nos casos que já eram
  `TooTight`.

### C2.2 — `need_mb` deixa de ser o tamanho do ficheiro

- **Mudança:** `hub.need_mb` = `bytes/1MiB + kv_est_mb(ctx, layers)` com
  `kv_est` explícito (`k=vram_estimate` no JSON). Sem `block_count` no
  `parse_gguf_header` (só lê magic/versão/contagens — `nos-core/src/gguf.rs:23`),
  a estimativa usa `NOSD_LLAMA_FIT_CTX` e um `layers` declarado (default 32) —
  **e diz que é estimativa**. Ler `block_count` da secção KV fica fora deste
  plano (ver "Não entra").
- **Prova:** `model.list` mostra `need_vram_mb` e `source:"estimate"`.

### C2.3 — Evict que descarrega

- **Achado:** `hub.rs:55` (`evict_coldest`) só faz `loaded=false`; não liberta
  nada.
- **Mudança:** `ModelHub::evict(&hotswap) -> Option<ModelSlot>`: `hotswap.unload()`
  e depois marca. Teste: após evict, `engine_name()=="cpu-echo"` e
  `loaded_count()==0`.
- **Prova:** unitário + log `hub evict slot=X engine=cpu-echo`.

### C2.4 — VRAM por placa, não somada

- **Achado:** `capacity.rs` soma `memory.free` de todas as GPUs.
- **Mudança:** `vram_free_mb` = **máximo por placa**; novo
  `vram_free_total_mb` para relatório; `serde(alias)` para o HUD não quebrar.
- **Prova:** fixtures de 1/2 GPUs; log de capacity com ambos os valores.

### C2.5 — Colisão de slots é um conflito, não uma substituição

- **Achado:** `hub.rs:35` (`register_path`) é last-write-wins; o boot regista
  todos os `.gguf` em 8
  slots → o 9B pode herdar o `GeneratorPro` do 4B.
- **Mudança:** `SlotState.candidates: Vec<PathBuf>`; `register_path` recusa
  substituir path diferente e regista conflito; `model.list` expõe
  `conflict:true`; `plan_slot` prefere o path declarado em `NOSD_LLAMA_MODEL*`.

---

## Fase 3 — Cliente `llama-server` (P1, independente)

### C3.1 — O retry do 400 volta a existir
- **Achado:** `post_chat` converte tudo em `Infer("llama-server http")`; o
  `ureq 2.12` devolve `Err(Status)` em 4xx/5xx, logo o ramo que testa
  `resp.status()` é código morto e o corpo do erro perde-se.
- **Mudança:** casar `ureq::Error::Status(code, resp)`; guardar
  `code` + 200 chars de corpo em `SwapError::Infer`/novo `Http{code,body}` e
  repetir sem `chat_template_kwargs` nesse caminho (`trim_diagnostic`, lição 30).
- **Prova:** servidor falso que responde 400 na primeira chamada; teste de
  integração no `nos-cortex` (ou `smoke` com `python3 -m http.server` substituído
  por um stub).

### C3.2 — Um só `n_predict`
- **Achado:** 64 no CLI (`llama_cli.rs:53`) vs 192 no servidor.
- **Mudança:** constante única em `nos-cortex` usada pelos dois.
- **Prova:** teste de paridade.

### C3.3 — Cap térmico: do env para o spawn (e de volta)
- **Achado (corrigido):** o consumidor existe — `main.rs:689-706` — mas escreve
  no `NOSD_LLAMA_NGL` **depois** de o motor estar a correr: não reduz o calor
  actual, só envenena o próximo spawn, e nunca repõe o valor quando arrefece.
- **Mudança:** `HermesState`/`nos-cortex` ganham `thermal_ngl_cap: AtomicU32`
  (0 = sem cap) escrito pelo tick de observe; `llama_server::spawn` aplica
  `min(ngl_pedido, cap)` **só** quando existe `-ngl` explícito (sem `-ngl`, o
  `--fit` já decide e o cap só aparece no log); o valor sobe e desce com o
  snapshot (sem sticky).
- **Prova:** unitário `cap_apply`/`cap_release`; log no QEMU com `cap=16` e
  `cap=none` após arrefecer (simulável com `NOSD_PRESENCE`).

---

## Fase 4 — Voz (P2)

### C4.1 — Voz compatível com o locale ou nada
- **Achado:** `find_piper_model` cai no primeiro `.onnx` (voz estrangeira
  silenciosa).
- **Mudança:** exigir prefixo de idioma (`pt`/`pt_BR`) no basename; sem match →
  `VoiceError::ModelAbsent(voz pedida)`, com `voice.status` a listar as vozes por
  idioma.
- **Prova:** unitários com dirs sintéticos; `voice.status` no host.

### C4.2 — STT: base antes de tiny, e a razão declarada
- **Mudança:** preferência `base > tiny` quando ambos existem; `voice.status`
  ganha `stt_model_reason`. Medir RTF de um WAV de 3 s no host e registar.
- **Prova:** WAV de `docs/evidence/` + `whisper-cli` real; número no commit.
- **Not-tested até metal:** taxa de acerto em fala real.

### C4.3 — Um backend, um WAV
- **Achado:** audiocpp + piper escrevem no mesmo temporário; o piper falhado
  trunca o bom resultado do audiocpp → espeak.
- **Mudança:** se audiocpp produz `wav_ready`, **não** correr piper; se o piper
  falhar, apagar o ficheiro antes do espeak escrever.
- **Prova:** unitário com binários falsos (sucesso/falha combinados).

### C4.4 — `aplay` por dispositivo real
- **Mudança:** listar com `aplay -l` e tentar os cartões analógicos disponíveis;
  `card_busy` (Subdevices 0/1) passa a mensagem própria e próximo cartão.
- **Prova:** host com 2 devices (um ocupado); metal: som audível (AWAITING_HW).

---

## Fase 5 — Fábrica (P2) — **exige decisão de infra antes do código**

### C5.1 — Construir com `nix build`, não com o PATH
- **Achado:** `scaffold::build_and_run` usa `cargo`/`python3` do PATH; o unit
  `nosd` (`modules/neural-os.nix:214`) não tem nenhum dos dois → na ISO toda a
  fábrica é `engine_absent`. O `nix` **está** no PATH e o `flake.nix` já é gerado.
- **Bloqueio real (a resolver primeiro):** `nix build` como utilizador `nosd`
  só funciona com `nix-daemon` a correr e `trusted-users` a incluir `nosd`
  (escrever no store é privilegiado). Sem isso, C5.1 não tem como passar.
- **Mudança:** `run_build` → se `nix` no PATH e o daemon for utilizável
  (`nix store ping`), `nix build --no-link --print-out-paths` e executar
  `out/bin/<name>`; senão fallback PATH (comportamento actual, declarado).
  `RunOutcome.tool` passa a dizer `nix` vs `cargo` vs `python3`.
- **Prova:** QEMU: `nosctl factory-build "app python de saudação"` → `ok`,
  `tool:"nix"`; idem Rust. Sem nix-daemon: `engine_absent` com a razão.
- **Not-tested até haver QEMU:** o caminho `nix build` em metal.

### C5.2 — Não destruir uma app existente
- **Mudança:** `write` devolve `Exists` se o directório já tem ficheiros do
  plano; RPC ganha `overwrite:false` por omissão.
- **Prova:** unitário (2 writes).

### C5.3 — `Lang::detect` sem código morto
- **Mudança:** procurar palavras (`rust|py|python|cargo`) no intent; remover o
  `parse(intent_inteiro)` que nunca casa.
- **Prova:** unitários existentes + 2 casos novos.

---

## Fase 6 — WASM / skills (P2)

### C6.1 — `max_pages` aplicado
- **Achado:** `HostState::with_policy` passa `memory_size`, `table_elements`,
  `instances` e ignora `policy.max_pages`.
- **Mudança:** limites de memória = `min(heap_bytes, max_pages × 64 KiB)` (e
  documentar a relação); teste com `(memory 4)` aceite e `(memory 8)` recusado.
- **Prova:** unitários `nos-wasm`.

### C6.2 — `aios_ai.infer` declarado como stub
- **Mudança (recomendada):** manter o stub e expor `infer:"stub"` em
  `skill.explain`/`self.status` — uma skill com `INFER` não promete inferência.
  Implementar a ponte para o motor é um item próprio (Fase 8, fora deste plano).
- **Prova:** campo novo no JSON + teste de contrato.

---

## Fase 7 — Energia (P3)

### C7.1 — RAPL de todos os packages
- **Mudança:** somar `intel-rapl:N/energy_uj` (N = packages) em vez de só `:0`;
  wrap por contador, não global.
- **Prova:** unitário com fixtures de 2 packages; host sem RAPL continua
  `absent` = 0 J.

---

## Ordem, gates e rastreio

**Ordem:** F1 → F2 → F3 → (F4, F5, F6, F7 em paralelo).

**Gate por fase** (uma linha de comando, sem `| head`):

```
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings \
  && cargo test --workspace && cargo check -p nos-jarbas --features slint-ui
```

Mais, quando a fase toca spawn/VRAM/voz/ISO: `nix-instantiate --parse` nos
ficheiros `.nix` tocados, `nix build .#nosd` (WSL) e
`bash scripts/gate-boot-to-jarbas.sh --stage MQEMU`.

| Achado (revisão 2026-11-06) | Tarefa | Prova |
|---|---|---|
| `-ngl` do boot anula o fit | C1.1 | QEMU journal + metal |
| 4B carregado duas vezes | C1.2 | unit + host 2 motores + metal |
| `loaded`/evict sem efeito | C2.3 | unit `engine==cpu-echo` |
| fit soma RAM+VRAM / need=ficheiro | C2.1, C2.2 | unit tabela + `model.list` |
| VRAM somada entre placas | C2.4 | fixtures |
| slot last-write-wins | C2.5 | unit `conflict:true` |
| retry 400 morto / status perdido | C3.1 | stub HTTP 400 |
| `n_predict` 64 vs 192 | C3.2 | paridade |
| cap térmico no env, sticky, pós-spawn | C3.3 | unit + log QEMU |
| saúde do spawn 30 s fixo | C1.4 | stub lento + QEMU |
| voz estrangeira silenciosa | C4.1 | unit |
| tiny antes de base | C4.2 | RTF medido |
| dois backends no mesmo WAV | C4.3 | unit |
| device de áudio fixo | C4.4 | host + metal |
| fábrica usa PATH, não nix | C5.1 | QEMU `tool:"nix"` |
| scaffold sobrescreve | C5.2 | unit |
| `max_pages` ignorado | C6.1 | unit |
| `aios_ai.infer` stub mudo | C6.2 | contrato |
| RAPL só package 0 | C7.1 | fixtures |

## Riscos

- **R1** (C1.1): build sem `--fit` cai para CPU — detecção `offloaded 0/N`,
  mitigação `-ngl` explícito documentado.
- **R2**: o `fit` do llama.cpp passa a ser o único árbitro da VRAM; perdemos
  controlo fino e ganhamos correcção. Aceitar.
- **R3**: `fit_model` muda de assinatura → 3 call sites + testes (âmbito fechado
  acima).
- **R4** (C5.1): `nix build` como `nosd` exige `nix-daemon` + `trusted-users`;
  sem isso, a fábrica continua honestamente `engine_absent` e o código de C5.1
  não vale a pena sozinho.
- **R5**: nenhuma fase desta lista se prova em metal neste host. O máximo
  atingível aqui é host + QEMU; metal fica `AWAITING_HW` e tem de estar escrito.

## Não entra neste plano

- Refactor do motor supervisionado por omissão (`nosdEngine.supervise` default).
- Ler o `block_count` da secção KV do GGUF (melhora `need_mb`, não é preciso
  enquanto o `--fit` decidir).
- Ponte `aios_ai.infer` → motor real (Fase 8, decisão de segurança própria:
  caps + quota + HITL).
- Mesh, M4/M5, Ring.

## Definition of done

Uma fase fecha quando: (a) o gate completo passa; (b) a prova da tabela está
gravada em `docs/evidence/` (comando + saída); (c) o commit tem `Tested:` e
`Not-tested:` com o que faltou (metal, som audível, GPU no cage); (d) nenhuma
linha deste plano fica "feita" só porque o teste unitário passou.

---

## Estado de execução — 2026-11-06

| Fase | Estado | Prova |
|---|---|---|
| F1 (C1.1–C1.4, C3.2, C3.3) | HOST OBSERVED | `INFERENCE_F1_2026-11-06.md`; 1021 testes |
| F2 (C2.1–C2.5) | HOST OBSERVED | `INFERENCE_F2_F7_2026-11-06.md`; 1029 testes |
| F3 (C3.1) | HOST OBSERVED | idem; 1031 testes |
| F4 (C4.1–C4.4) | HOST OBSERVED, metal pendente | idem; 1037 testes no conjunto |
| F5 (C5.2, C5.3) | HOST OBSERVED | idem |
| F5 (C5.1) | **PARCIAL** — código pronto, infra em falta | `nix build` como `nosd` pede `nix-daemon` + `trusted-users`; sem isso cai no PATH de propósito |
| F6 (C6.1) | HOST OBSERVED | `effective_memory_bytes` |
| F7 (C7.1) | HOST OBSERVED, wrap ainda global | soma de packages; wrap por contador é residual |

Fica dito sem rodeios: **nenhuma** fase tem QEMU nem metal. O que este host
prova é o gate (fmt/clippy/testes) e as funções puras; o argv do motor, a
segunda cópia do 4B, o som no `aplay` e a VRAM real de `nvidia-smi` continuam
`AWAITING_HW`.
