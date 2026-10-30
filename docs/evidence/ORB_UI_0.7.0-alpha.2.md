# ORB UI — evidência (0.7.0-alpha.2, 2026-10-30)

Âmbito: `packages/nosd/crates/nosd/static/orb_lab.html` (HUD cinematic),
`KIOSK_METHODS`, `health.mesh_nodes` e `MeshEngine::topology()`.

## O que foi provado

| Item | Resultado | Como |
|------|-----------|------|
| Build/verify | **PASS** | `cargo fmt --all --check`, `cargo clippy -D warnings`, `cargo test --workspace` via `run_verification` → `VERIFY_OK stack=rust` |
| Topologia da mesh | **PASS** | `cargo test -p nos-mesh topology_reports_local_identity_without_peers` (id 15, Worker, 8 CPU, 0 peers) |
| Layout 1283×873 | **PASS** | Chromium: rail esquerda 737/737, direita 737/737; painéis 219+171+153+152; CC 86; dock 74 |
| Layout 1280×720 (iframe) | **PASS** | rail esquerda 591/591 (187+154+105+121), direita 591/591; mesh SOLO oculta a 06; com 1 peer: atividades 277 + mesh 206 + auth 92 = 591 |
| CC/chat minimizado | **PASS** | CC só com STT (VOCÊ) e TTS (JARBAS); texto/sistema no `HISTÓRICO`/feed; modo texto só com `TXT`/falha de mic |
| CC modo voz/texto | **PASS** | modo voz: CC só `voice`; texto no `HISTÓRICO`. Voz desabilitada (`TXT`/mic em falta): `teste barra central` apareceu na CC e `HISTÓRICO · 2`; `lastVoice` persiste enquanto há voz |
| Mesh 06 condicional | **PASS** | mesh off (`peers=0`) → `#meshPanel` `display:none`; `05 · ATIVIDADES` expandiu para 624 px (1283×873) |
| Energia CPU/GPU | **PASS (unit + host)** | `EnergySplitMeter`: 2 J CPU + 10 J GPU = 12 J, fontes `rapl`/`nvml`; host Windows sem RAPL: CPU `—`, GPU `nvml` 17.6 W/48 °C, `MED`, `Σ` J |
| Termal/fan | **PASS (advisory)** | hwmon read-only; host sem fan sensor: `VENT — · 48°C`, `HOLD`; limiares `FAN +/++/−` testados; **não** actua ventoinhas |
| Caixa 03 sem crescer | **PASS** | 720p: `#energyPanel` **105 px** (= altura antiga); left 591/591. 873: 132 px < 153 px antigo |
| Mesh 2 instâncias | **PASS** | A `:7700` N15 Master; B `:7702` N16 Worker; `peers=1`, `pinned=1`, `TOFU SETTLED`; janela 06 `LIVE`, drawer lista `MST/N15` + `WRK/N16 · PIN` |
| Orb mesh mini-orbs + rails | **PASS (foto)** | Peer N16 aparece como mini-orb ciano (WRK) à distância, ligado por trilho bidireccional (duas pistas, travessas, segmentos e seta); local N15 MST no centro. Foto: `artifacts/orb_ui_mesh_2nodes.jpeg` |
| HW default cinematic | **PARSE OK** | `images/hw.nix` → `uiMode = lib.mkForce "cinematic"`; `nix-instantiate --parse` OK; `nix eval` devolveu `cinematic`. Build `iso-hw`/metal AWAITING_HW |
| iso-hw all models | **BUILD PENDING (alpha.2)** | symlinks 0.8B Q4/Q8, 4B Q4/**Q6**, 9B Q4 em `/var/lib/nosd/models`; rebuild `nix build .#iso-hw` a correr; hashes abaixo a preencher |
| Tier 4B Q6 (6 GB) | **PASS (unit)** | `six_gb_profile_prefers_q6_over_q4`: 6 GB VRAM + 5 GiB RAM → `Qwen3.5-4B-Q6_K.gguf` (tier 3) antes do Q4; `low_ram_tier3_still_picks_q4`: 4,4 GiB → Q4; `accept(3)` escolhe o candidato alcançável, não o primeiro da lista |
| Contexto automático | **PASS (código + help)** | `llama-server --help` (llama.cpp 0.6.0) confirma `-fit on` e `-fitc N` (default 4096); `spawn` passa `--fit on --fit-ctx` (mínimo default **2048**, `NOSD_LLAMA_CTX` explícito manda); `iso-hw` deixa de forçar 4096 |
| HITL Wi‑Fi | **PASS** | `nosctl ui-open hitl_wifi --grant-kind wifi.connect` → modal `MEDIUM`, campos SSID/senha, `CANCELAR` → `pending=0` |
| HITL alto | **PASS (modal)** | `nix.apply` → `HIGH`, checkbox de compreensão, botão `MANTER PARA AUTORIZAR`; hold não concluiu com a tab invisível (rAF throttled), não é regressão do handler |
| Mic primeiro | **PASS (fake stream)** | `MediaStreamDestination` substituiu `getUserMedia`: `MIC ON` depois de o STT falhar; vídeo negado não desligou o mic; UI caiu para TXT com `STT falhou · use TXT` |
| T/S | **PASS (estimativa)** | turno texto: `~93.0 t/s` com `title` “estimativa end-to-end · 14 tokens (chars/4) em 0.5s · não são timings do engine” |
| Mesh SOLO | **PASS** | `N15 · WRK`, `0 peers`, `0 pin`, `TOFU —`, `FRAG 0 B`; janela 06 oculta; drawer do marcador mostra o nó local e órbita |

## ISO 0.7.0-alpha.2 (hw)

Estado: **BUILD PENDING** — a ISO alpha.1 (`1ef16b…b655`) não inclui o 4B Q6
nem o `--fit`; os valores abaixo só entram depois do rebuild.

- Link de build: `result-iso-hw` (ignored pelo git)
- Bytes/sha256: **a preencher** após `nix build .#iso-hw` (alpha.2)
- Volume ID mantém `NIXAIOS051` (não é derivado da versão); `version=0.7.0-alpha.2`.
- Modelos no ISO (5): `Qwen3.5-0.8B-Q4_K_M.gguf`, `Qwen3.5-0.8B-Q8_0.gguf`,
  `Qwen3.5-4B-Q4_K_M.gguf`, `Qwen3.5-4B-Q6_K.gguf`, `Qwen3.5-9B-Q4_K_M.gguf`;
  default 0.8B Q4; 4B/9B por `brain.plan/accept` (9B exige ring).
- USB único: `C:\Users\msrov\Downloads\nixaios-0.7.0-alpha.2-hw.img` (ISO +
  FAT32 4G `NIXAIOSLOG`) — bytes/sha256 **a preencher** após `pack-hw-usb.sh`.
- Gravar em **DD**, nunca modo ISO.

## Não provado

- **Metal/QEMU** com câmara, microfone e Piper reais — AWAITING_HW. O host de
  bancada não tem `arecord`/`pw-record` nem TTS; a UI declarou `VOZ PARCIAL` e
  abriu TXT (honestidade ADR-0113). O engine do lab é `cpu-echo` porque
  `jarbas-data/models/` não tem GGUF (fallback honesto; T/S não representativo).
- **Mesh com >2 peers / multi-host / NAT / Wi-Fi** — o teste local provou 2
  instâncias (N15 Master, N16 Worker, pinned/TOFU SETTLED); frota real e
  topologias MST/MEM/CPU maiores continuam AWAITING_HW.
- **HUD Slint nativo** — não foi reescrito nesta tranche; o fallback
  `uiMode=slint` mantém o HUD anterior.
- **First boot / brain.plan / metal** — a ISO 0.7.0-alpha.2 vai conter os 5
  GGUFs, mas não foi arrancada em metal/QEMU nesta sessão; o 4B Q6 e o contexto
  máximo do `--fit` não foram medidos numa 3050 real (AWAITING_HW).

## Ficheiros

- `packages/nosd/crates/nosd/static/orb_lab.html` — HUD (CC, dock de voz, HITL
  modal, mesh, teia cognitiva, T/S).
- `packages/nosd/crates/nosd/src/rpc/http_lab.rs` — `cognitive.trace` na
  allowlist + contrato.
- `packages/nosd/crates/nos-mesh/src/lib.rs` — `topology()` + teste.
- `packages/nosd/crates/nosd/src/rpc/health.rs` — `mesh_nodes`.
- `docs/ORB_UI.md` — desenho e atalhos.
