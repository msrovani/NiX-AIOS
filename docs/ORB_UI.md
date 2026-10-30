# ORB UI — JARBAS cinematic HUD

Estado: **in-tree (0.7.0-alpha.2)**. Alvo: HUD do kiosk cinematic
(`NOSD_ORB_LAB=127.0.0.1:7701` → Chromium/cage). O HUD Slint nativo continua
a ser o fallback `uiMode=slint`; esta tranche não o reescreve.

## Layout

```
TOPBAR   JARBAS · estado · LINK LOCAL · relógio · CAM · SND · ?
┌────────────┬──────────────────────────────────┬───────────────┐
│ CORE       │                                  │ ATIVIDADES    │
│ TELEMETRIA │              ORB                 │ (feed OPAR)   │
│ ENERGIA    │                                  │ MESH          │
│ VISÃO      │  CC / closed captions            │ AUTORIZAÇÃO   │
│            │  [MIC] VOZ PRONTA … TXT AUTO     │               │
└────────────┴──────────────────────────────────┴───────────────┘
FOOTER   HITL server-authored · MIC/TXT/ESC/H · DIAG · NIXAIOS 051
```

- **Voz primeiro**: o microfone é o controlo principal. O compositor abre em
  modo voz; o campo de texto só aparece com `TXT` (ou `/`). Se o microfone
  falhar, o modo texto abre automaticamente.
- **Áudio antes de vídeo**: `getUserMedia({audio})` é pedido primeiro; a câmara
  é pedida à parte e uma falha de vídeo **não** desliga o microfone.
- **CC**: em **modo voz**, a faixa central mostra apenas o transcript do STT
  do utilizador (VOCÊ) e a saída do TTS do JARBAS (JARBAS). Texto digitado e
  mensagens de sistema ficam no feed de atividades e no `HISTÓRICO`. Se a voz
  está **desabilitada** (mic/TTS em falta ou TXT aberto), a CC passa a mostrar
  o último texto do chat — o operador nunca fica sem retorno. O ecrã não é
  ocupado por um painel de chat permanente.
- **HITL**: cada pedido abre um modal com tier (`trivial/low/medium/high`),
  título, corpo, grant kind e campos dinâmicos. `high` exige checkbox de
  compreensão + *manter* (~700 ms) para autorizar. O nonce nunca chega ao
  browser; o HUD só envia `ui.action`.
- **Mesh**: a janela `06 · MESH` só aparece quando há **peers > 0** (rede
  mesh real). Sem peers o cartão desaparece e a janela de atividades ocupa o
  espaço; o marcador `MESH` no orb mantém o drawer de diagnóstico com
  `SOLO`/`OFF`. Com rede, o painel mostra um grafo 3D pequeno (local + peers):
  cor = papel (`MST` master/âmbar, `WRK` worker/ciano, `MEM` memória/violeta,
  `CPU` compute/verde). No orb central, cada peer é uma **mini-orb distante**
  na cor do papel, ligada por um **trilho de dados bidireccional**: duas pistas
  (envio/recepção) com travessas, segmentos animados e seta no destino; a
  velocidade sobe com `last_frag`/`pending_fracks`. Anel sólido = peer pinado
  (TOFU); tracejado = ainda não fixado. O organograma detalhado (id, papel,
  CPU, MiB, PIN) está no drawer.
- **Energia (03)**: CPU e GPU em colunas separadas — CPU por RAPL
  (`cpu_package`), GPU por NVML (`nvidia-smi power.draw/temperature.gpu`); duas
  linhas no mini-gráfico (ciano CPU, violeta GPU). Em baixo: ventoinha via
  hwmon (`fan*_input`/`pwm*`), temperatura máxima e recomendação **advisory**
  `FAN +` / `FAN −` / `HOLD` / `SEM SENSOR`. Limiares: ≥88 °C `FAN ++`, ≥78 °C
  `FAN +`, ≤55 °C com fan ≥35 % `FAN −`; sem sensor não se inventa. O `nosd`
  **não** actua ventoinhas; a caixa 03 mantém a altura (no 720p fica compacta).
- **T/S**: a linha `T/S` mostra `~N t/s` **estimado end-to-end** (chars/4 ÷
  tempo do turno). O `title` diz explicitamente que não são timings do engine;
  sem turno medido fica `—`. Não inventar tokens/s (manifesto Cortex/Hermes).

## Cores do orb

| Cor | Significado |
|-----|-------------|
| ciano | idle/online |
| água | listening |
| âmbar | thinking / autorização pendente |
| ciano-claro | speaking |
| violeta | dreaming / memória |
| vermelho | alert/error |

As linhas da teia são pensamento (densidade sobe com `thinking` e OPAR); os
nós são pesquisas/memória; os bursts de pesquisa vêm de `cognitive.trace`
(domínios `seek/search/recall/...`). Uma autorização pendente acrescenta nós
âmbar no anel externo; um grant aplicado dispara um anel verde.

## Dados (sem nova superfície de escrita)

- `health` — inclui `mesh_nodes` (topologia read-only: local + peers, papéis,
  capacidade, TOFU/idade). `MeshEngine::topology()` não expõe chaves.
- `ui.list` / `ui.pending` — janelas e tiers; `ui.action` é a única mutação.
- `cognitive.trace` — últimos actos OPAR **sem notas/PII** (domínio + lengths).
- `observe.energy` — joules/watts/source/scope (absent = não medido).
- `orb.get` / `orb.sense` — estado e nível/movimento.

`cognitive.trace` foi adicionado à `KIOSK_METHODS` (cap NONE) com o teste de
contrato existente; `health.mesh_nodes` não abre método novo.

## Atalhos

- `MIC` — falar (STT local); em caso de falha, abre TXT.
- `TXT` — mostrar/esconder o campo de texto.
- `Enter` — enviar no modo texto; `Esc` — cancelar HITL / fechar texto.
- `/` — abrir o modo texto; `H` — primeira autorização pendente.
- `SND` — mute; `CAM` — câmara (movimento) on/off.
- `PAT` — ciclar padrão do orb (no painel CORE).
- `AUTO` — loop voz → STT → resposta → voz (recusa se não houver TTS).

## Correr na bancada

```bash
cd packages/nosd
cargo run -p nosd -- --foreground --tcp 127.0.0.1:7700 \
  --data-dir ./data --orb-lab 127.0.0.1:7701
# noutro terminal:
cat ./data/jarbas-ui-bootstrap.token
# browser: http://127.0.0.1:7701/#bootstrap=<chave>
```

No kiosk: `jarbasCage.uiMode = "cinematic"` (ou `auto` com GPU validada) liga
`NOSD_ORB_LAB=127.0.0.1:7701`.

**HW real**: `images/hw.nix` força `uiMode = "cinematic"` (Chromium `--kiosk`,
fullscreen, sob cage) — o orb é a primeira interface gráfica depois do
firstboot/hotswap. A ISO de release e o QEMU continuam `uiMode = "slint"`; se o
Chromium sair/GPU falhar, o script faz fallback para Slint. A `iso-hw`
0.7.0-alpha.2 leva **todos os GGUFs pinados** (0.8B Q4/Q8, 4B Q4/**Q6**, 9B Q4)
em `/var/lib/nosd/models`; o default é 0.8B Q4 e os maiores só activam por
`brain.plan/accept` (HITL; o 9B exige ring). O **4B Q6_K** é o tier para 6 GB
(escolhido antes do Q4) e o `llama-server` arranca com `--fit on`/`--fit-ctx`:
sem `-c` fixo, escolhe o maior contexto que cabe (mínimo default 2048).

## Testado (host, 2026-10-30)

- Build `cargo build -p nosd`; `cargo test -p nos-mesh topology_...`; verify
  completo (`fmt --check` + `clippy -D warnings` + `cargo test --workspace`)
  **VERIFY_OK**.
- Chromium 1283×873 e iframe 1280×720: rails sem overflow (esquerda 591/591,
  direita 591/591), CC e dock de voz; mesh `SOLO N15 WRK`; HITL Wi‑Fi
  `MEDIUM` criado e cancelado; HITL `nix.apply` `HIGH` com checkbox/hold;
  chat texto → `~93 t/s est`; microfone fake (`MediaStreamDestination`) fica
  `MIC ON` mesmo com vídeo negado; STT ausente cai para TXT com honestidade.
- **Duas instâncias nosd** (`:7700` N15 Master + `:7702` N16 Worker, UDP
  19000/19001): `peers=1`, `pinned=1`, `TOFU SETTLED`; a janela 06 aparece
  `LIVE` e o drawer mostra `MST/N15` e `WRK/N16 · PIN`. Box 03 mantém 105 px
  no 720p com CPU `—` (Windows sem RAPL), GPU NVML 17.6 W/48 °C, `HOLD`.
- **`nix build .#iso-hw`** alpha.1 (2026-10-30) PASS: ISO 12 456 597 504 B,
  sha256 `1ef16b277fb2e291d4350fb286619e7c33b35c466816ba5d4fd979374081b655`,
  com 0.8B Q4/Q8 + 4B Q4 + 9B Q4. `.img` único (ISO + FAT32 4G)
  16 752 050 176 B, sha256
  `fe97dfe42959066e589993f7a5180c725f812f3ec4a2264fc680c97fdb716f05`.
  O rebuild **alpha.2** (5 GGUFs com Q6) fica registado na evidência; os
  hashes alpha.1 **não** valem para alpha.2. Metal/firstboot AWAITING_HW.

## Não testado

- Metal/QEMU com câmara + microfone + Piper reais (AWAITING_HW).
- Mesh com peers reais e papéis MST/MEM/CPU (o host tem mesh off; o grafo
  mostra `SOLO`).
- HUD Slint nativo (não foi reescrito nesta tranche).
- Captura de ecrã do desktop (a janela do browser não estava visível).
