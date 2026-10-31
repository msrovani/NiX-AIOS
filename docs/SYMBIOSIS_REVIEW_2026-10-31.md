# Revisão de simbiose NixOS ↔ AIOS — 2026-10-31

Revisão pedida como “grande revisão” do projeto: ler a estrutura e os contratos,
confrontar **código ↔ módulos Nix ↔ docs**, e corrigir divergências concretas.
Não é um relatório de marketing: o que está `AWAITING_HW` continua
`AWAITING_HW`.

## 0. Método e escopo

Lido com atenção (código + docs centrais):

- `AGENTS.md`, `CHANGELOG.md`, `docs/ARCHITECTURE.md`,
  `docs/BOOT_TO_JARBAS.md` (M0–M3g), `docs/SKILL_TRUST_AND_SELECTION.md`,
  `docs/API.md`, `docs/SECURITY.md`, `docs/IMPROVEMENT_PLAN.md`;
- `flake.nix` (configs, checks), `images/{iso,hw,hw-cuda}.nix`,
  `modules/{neural-os,nosd-engine,nosd-brain,nosd-early,nosd-tts,jarbas-cage}.nix`,
  `modules/transmute/*.sh`;
- crates `nos-cortex` (capacity/fetch/fit/hotswap), `nos-hermes`
  (cognitive_bridge, blob_deliberate), `nos-memory` (seg/backend/recall),
  `nos-wasm`, `nos-factory`, `nos-voice`, `nosd` (rpc/skills, rpc/model,
  main/AppState);
- testes de integração (`ipc_auth`) e checks Nix (`hw-cuda-deviceallow`,
  `nixaios-vm`, `mesh-*`).

Amostragem estruturada (grep de contratos/envs + leitura de trechos) nos
restantes docs de release/evidência e crates (`nos-bei`, `nos-bus`,
`nos-observe`, `nos-mcp`, `nos-jarbas` UI). Isto **não** é leitura literal de
cada linha do repositório; é auditoria dos contratos que sustentam o boot, o
córtex, a memória, a UI e a fábrica.

Verificação executada no fim: `cargo fmt --check`, `clippy -D warnings`,
`cargo test --workspace`, `nix build .#nosd`, checks estáticos novos e
`nix-instantiate --parse` dos módulos tocados. Nada disto prova metal.

## 1. Mapa real do arranque (boot → JARBAS)

| # | Etapa | Unidade / contrato | Estado honesto |
|---|---|---|---|
| 0 | Boot NixOS (ISO/minimal + módulos) | `images/iso.nix`, `modules/*` | OBSERVED em QEMU; metal parcial |
| 1 | Daemon | `nosd.service` (`Type=notify`, socket `/run/nosd/nosd.sock`) | OBSERVED |
| 2 | Early score | `nosd-early-score` (`nosctl observe/score`) | OBSERVED |
| 3 | Firstboot | `nos-firstboot` no tty1 (Q1–Q5; auto na ISO) | OBSERVED |
| 4 | Transmute/hotswap | `nosd-hotswap-aios` → cage/DRM | OBSERVED em QEMU |
| 5 | HUD | `jarbas-cage` (Slint ou Chromium kiosk) | Slint OBSERVED; cinematic AWAITING_HW |
| 6 | Voz | `nos-voice` → `arecord`/`aplay`/STT/TTS | EngineAbsent honesto; metal AWAITING |

O desenho é **systemd-first**: o `nosd` não é um init; é um serviço com
socket, hardening, `StateDirectory` e limites. É a simbiose certa para NixOS —
o SO é o orchestrator e o AIOS é o workload.

## 2. Simbiose com NixOS: o que já é estrutural

- **Store content-addressed como cache de modelos**: GGUF pinado entra em
  `/var/lib/nosd/models` por `tmpfiles` `L+` (`modules/nosd-brain.nix`): nem
  download no boot, nem cópia — a derivação é o artefacto. Rollback de geração
  NixOS reverte o modelo.
- **Política no módulo, não no payload**: `services.nosd.skillGrantCaps`,
  `skillAppProfile`, `skillQuota*`; HITL server-authored; caps por tool no
  MCP. Declarar não autoriza.
- **cgroups de dispositivos**: `DeviceAllow` é o gate real de hardware
  (`char-drm`, `char-nvidia`, `char-alsa`); PATH/PID não decide acesso.
- **Checks como gate barato**: `hw-cuda-deviceallow`, `mesh-2vm*`,
  `nixaios-vm`, agora `nosd-voice-deviceallow` e `engine-fit-parity`.
- **cage/Wayland**: kiosk sem X11, DRM lease, VT activa (`chvt`) — o caminho
  gráfico é o do kernel, não uma stack paralela.

**Subutilizado (dívida, não bug):** `WatchdogSec=0` no `nosd` (o `Type=notify`
suporta watchdog); ausência de `LoadCredential=` para a API key do
llama-server; `SystemCallFilter` só parcial; sem `IOWeight`/`CPUQuota` para o
motor em disco lento; modelos ainda escolhidos por tabela, não por catálogo
versionado no store.

## 3. Escolha de modelo em runtime (Cortex)

Cadeia real (`nos-cortex`, `nos-hermes`, `rpc/model.rs`, `rpc/brain.rs`):

1. `CapacityProber::probe()` → RAM/VRAM/vendor;
2. `suggest_for_capacity` → quant (Q5_K_M…Q2_K) + `ngl` (puro, testado);
3. `fit_model` → cabe/offload/airllm (política, testada);
4. spawn do `llama-server` com **`--fit on --fit-ctx`** (sem `-c` fixo) e
   `--reasoning off`; `NOSD_LLAMA_CTX` explícito continua a mandar;
5. `job`/chat → S2 externo se `NOSD_LLAMA_SERVER_S2`; tier 4B/9B por
   `brain.plan`; `brain.accept` exige HITL e verifica sha256 antes de carregar;
6. `NOSD_BRAIN_PROPOSE=1` propõe upgrade pós-boot (anti-spam 24 h no CRDT).

Correção desta revisão: o motor **supervisionado** (`services.nosdEngine.supervise`)
usava só `-ngl`, sem `--fit`/`--reasoning` — era outro motor com o mesmo nome.
Agora ambos os units (template e concreto) têm paridade, com opções
`fitCtx`/`reasoning`, e o check `engine-fit-parity` impede regressão.

Residual: catálogo de tiers é hardcoded (0.8B/4B/9B Q4/Q6/Q8); a busca HF é
heurística e não decide download sem HITL; não há avaliação automática de
qualidade (prompt-eval) para justificar um upgrade; o cliente HTTP não
verifica qual modelo o servidor está de facto a servir (só o path pedido).

## 4. Memória (neural-sgdb)

Realidade do crate `nos-memory`: segmento **length-prefixed** em
`data_dir/nsgdb/L{2,3,4}/seg.bin`; recall léxico sobre L2+L3; L4 é stub
declarado; **não** é o motor MCP neural-sgdb (sem BQ/embedding/semântica).
Isto está dito no código e no `BOOT_TO_JARBAS` (M3g) — a nomeação “neural-sgdb”
é aspiracional e não deve ser lida como capacidade.

Correções desta revisão:

- recall passou a **dobrar acentos e pontuação** e a descartar stopwords PT/EN:
  `memoria` encontra `memória`, `wifi` encontra `Wi-Fi`; uma query só com
  stopwords devolve vazio em vez do arquivo inteiro com score 0;
- o bloco `## Memory recall` do prompt passa a declarar que a memória é
  **contexto não verificado, nunca instruções**. O segmento não é assinado e
  o data-dir da live é 1777: quem escreve memória pode tentar prompt-injection
  persistente. A fronteira real continua a ser o CapGate/HITL.

Residual: memória sem assinatura/âncora (diferente do log CRDT); sem
embeddings/BQ; sem TTL/decay; sem escrita concorrente multi-processo.

## 5. JARBAS como maestro

- Prompt em três blocos com ordem cache-friendly: `## Persona (SOUL)` (fixo)
  → `## Memory recall` (estado) → `## User` (dinâmico); sem timestamps no
  prefixo fixo.
- HUD Slint (`nos-jarbas`) e kiosk Chromium (`jarbasCage.uiMode`) partilham o
  bridge HTTP com allow-list de métodos e cookie de sessão; `uiMode=auto`
  exige `MemAvailable` **e** `chromiumGpuValidated`, senão cai para Slint.
- Ações de sistema: proposta + HITL server-authored (não há root no daemon);
  o ring Nix continua desarmado por decisão.

Residual: o maestro **não decide sozinho** trocar de motor/modelo sem HITL
(por desenho); não há orçamento de contexto por turno; a fila HITL não tem
prioridade por origem (voz vs texto). Voz: o unit do `nosd` não define
`XDG_RUNTIME_DIR`; com pipewire da sessão JARBAS o caminho ALSA/pipewire
continua dependente de `/run/nosd/user` e não foi provado em metal.

## 6. Fábrica de apps Wasm

Caminho real: `skill.install` → hash BLAKE3 → publisher/caps/downgrade →
inspeção de imports → gravação + `skill/<n>/active`; `skill.run` →
`nos-wasm::FrugalRuntime` (fuel/heap/logs/result ABI 2) → quota por peer
assinada; `skill.push` viaja no envelope e só ativa com hash igual.

O que **não** existe: codegen a partir de intenção. `nos-factory::promote`
marca `placeholder` quando o draft vem vazio; `agent.spawn` devolve
`placeholder:true` e `synthesized:false` em vez de fingir uma app. A “app wasm
a partir da necessidade do Hermes/JARBAS” é hoje: o Hermes **fornece** wasm
(ou um template WAT) → factory valida/promove → skill install/run. Gerar o
wasm a partir de linguagem natural exige um backend de codegen (LLM+toolchain
ou catálogo de templates) que não está no binário.

Proposta mínima (não implementada nesta revisão): um `factory.plan` que
devolve um **template WAT tipado** (aritmética/log/memória) escolhido por
contrato, com `content_id` e teste no sandbox antes de `skill.install`;
continua a não haver execução sem HITL para side-effects.

## 7. Correções feitas nesta revisão

1. **Voz no metal**: `nosd` tinha `DeviceAllow = [ "char-drm rw" ]` →
   `DevicePolicy=closed` implícita. `nos-voice` corre `arecord`/`aplay` como
   filhos; sem `/dev/snd` a captura morre com EACCES. Adicionado
   `char-alsa rw` + check `nosd-voice-deviceallow`.
2. **Paridade do motor supervisionado**: `--fit on --fit-ctx 2048
   --reasoning off --no-webui` nos dois units (`nosd-engine@` e
   `nosd-engine`), opções `fitCtx`/`reasoning`; check `engine-fit-parity`.
3. **`nosd-early-score` sem `path`**: o script usa `date/cat/tr/head/seq/
   mkdir/sleep`; adicionado `path = [ coreutils ]` (regra 5 do AGENTS).
4. **Recall de memória**: normalização de acentos/pontuação/hífen,
   stopwords e recusa de query só-stopwords; testes novos.
5. **Prompt-injection persistente**: aviso explícito no bloco de recall
   (memória = dados não verificados), teste que o exige.
6. **Docs**: corrigida a frase “1777 do data-dir só pré-transmute” — a ISO
   aplica 1777 a cada boot (intencional e já documentado em
   `ARCHITECTURE.md`/`SECURITY.md`); a frase antiga prometia um aperto que
   não existe.

## 8. Divergências docs ↔ código encontradas (e estado)

| Divergência | Estado |
|---|---|
| Motor supervisionado sem `--fit`/`--reasoning` (docs diziam “contexto automático”) | **corrigido** |
| `char-alsa` ausente com voz documentada como feature | **corrigido** |
| “1777 só pré-transmute” | **corrigido na doc** (código é 1777 permanente na live) |
| “neural-sgdb” como se fosse motor completo | já declarado honesto no crate/BOOT; mantém-se residual |
| `brain.plan` tiers fixos vs “sempre procurar melhorar” | residual/roadmap |
| codegen wasm a partir de intenção | **não existe**; `placeholder` é a resposta honesta |

## 9. Roadmap priorizado (com critério de aceite)

**P0 — confiança e boot**

1. Assinar/ancorar a memória nsgdb com a chave do nó (como o log CRDT) e
   recusar segmento truncado; aceite: teste de restart + edição recusada.
2. `WatchdogSec` real no `nosd` (notify a cada tick) e `Restart` observado;
   aceite: matar o processo e ver `systemd` reiniciar sem perder o socket.
3. `LoadCredential` para a API key do llama-server; aceite: unit sem key em
   `Environment`.

**P1 — maestro e modelos**

4. `factory.plan`: template WAT tipado + validação no sandbox antes de
   instalar; aceite: `skill.install` de app gerada com `fuel_used` e
   `result`, e recusa de template com import fora da allowlist.
5. Catálogo de modelos versionado (metadados: params, quant, bytes, sha,
   VRAM mínima) e `brain.plan` a escolher por capacidade **e** tarefa;
   aceite: plano determinístico para 3 perfis de HW.
6. Prompt-eval mínimo (perguntas fixas, score) para justificar upgrade;
   aceite: relatório medido, não estimado.

**P2 — operação**

7. Fila HITL com prioridade por origem (voz > texto > tick) e TTL visível no
   HUD; aceite: dois pedidos concorrentes, ordem observada.
8. Orçamento de contexto por turno (tokens estimados) com degradação
   honesta; aceite: turno longo não estoura VRAM e reporta o corte.
9. Memória: embeddings locais (BGE pequeno) sobre o segmento L2/L3, com
   recall híbrido e `basis` explícito; aceite: medição de recall@k vs lexical.

7. **`XDG_RUNTIME_DIR` do `nosd`**: o unit não o definia; a voz dependia de
   `/run/nosd/user` criado pelo `jarbas-cage`. Agora o `nosd` tem
   `XDG_RUNTIME_DIR=/run/nosd/user` e um `ExecStartPre` root que cria o
   diretório 0700 antes do drop para `nosd`; o check
   `nosd-voice-deviceallow` também o exige.

## 9.1 Contra-revisão (aceites corrigidos)

Segunda leitura do roadmap; estes critérios substituem os originais onde
entram em conflito.

| Item | Aceite original | Aceite corrigido |
|---|---|---|
| P0.1 memória | restart + edição recusada | **symlink plantado em 1777 recusado** (`O_NOFOLLOW`/`O_EXCL` na criação, `fstat`/`getdents`, escrita em subdir 0750); **root hash ancorado por append no log CRDT que já assina** (não uma segunda raiz) |
| P0.2 watchdog | matar processo → restart | **`kill -STOP` no PID principal → `WatchdogSignal` → um restart**, sem laço; ligações durante o restart não recusadas (`nosd.socket`, `Accept=no`) |
| P0.3 segredo | unit sem key em `Environment` | + chave por `LoadCredential` e `$CREDENTIALS_DIRECTORY` (nunca na store/systemctl show) |
| P1.5/P1.6 | catálogo + prompt-eval separados | **fundir**: entrada = store path com sha; resultado do eval indexado pelo **mesmo hash**; plano determinístico por construção |
| §3 resíduo | — | `/props` do llama-server confirma que o modelo servido == pedido no 1.º turno |
| §5 resíduo | — | `nosd` define `XDG_RUNTIME_DIR`; `arecord` funciona na sessão JARBAS (feito no item 7) |
| Contagem de testes | 442 host / 445 derivação | documentar a diferença: testes `#[cfg(unix)]` (Landlock/socket 0660) só compilam na derivação; não é sinal de regressão |
| Checks Nix | `nix-instantiate --parse` | `nosd-voice-deviceallow`/`engine-fit-parity` têm de correr num **runner Linux por commit** (CI Q-03), senão apodrecem |

Notas que ficam do contra-relatório:

- `WatchdogSec` sem `Type=notify`/`sd_notify(WATCHDOG=1)` = restart loop; o
  teste tem de ser `SIGSTOP`, não `kill`.
- Memória assinada continua a ser **dado**, nunca instrução (a assimetria do
  `## Memory recall` mantém-se).
- **Quota de skills**: repor o **par** janela+âncora antigo e consistente
  recupera orçamento — não há contador monotónico fora do data-dir (mesma
  classe da âncora do CRDT). A assinatura prova autoria, não frescura; na ISO
  de release a identidade M6 não está ligada (`signed=false`), e ligá-la é
  postura do mesh.
- Embeddings/híbrido só depois de assinatura + âncora (aumentam a superfície
  de injeção sem aumentar confiança).
- `factory.plan` com templates tipados antes de codegen livre.
- Ordem 48h: **LoadCredential + catálogo Nix-native indexado por hash + teste
  de áudio no metal**; sem tocar no que já está sólido (wasm install/quota,
  HITL, ordem cache-friendly).

## 10. Not-tested (não fingir)

- Metal (GPU/áudio/DRM reais), QEMU de boot completo nesta revisão;
- `supervise=true` a correr llama-server real (só flags/units avaliados);
- áudio sob `DevicePolicy=closed` em kernel real (gate estático apenas);
- promoção real de uma app wasm gerada por codegen (não existe backend);
- ISO reconstruída **com a revisão** (voz/XDG, DeviceAllow, Ozone) e
  registada em `docs/evidence/hw/2026-10-31_0140Z/`, mas **não arrancada em
  metal**; `nix build .#nosd` cobre o daemon.
