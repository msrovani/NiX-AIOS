# Sessão 2026-10-30 — ORB UI 0.7.0-alpha.1

## O que ficou

- HUD cinematic (`static/orb_lab.html`) voz-primeiro: mic grande, áudio antes
  da câmara, CC central (voz: só STT/TTS; texto quando a voz está off),
  `HISTÓRICO · N` no drawer.
- HITL modal: tier server-authored (`ui.pending`), campos dinâmicos, checkbox
  + hold-to-authorize no high; nonce nunca no browser.
- Orb: teia de pensamento/pesquisa, cores por estado, mini-orbs dos peers da
  mesh na cor do papel + **trilhos de dados bidireccionais** (duas pistas,
  travessas, segmentos, seta; velocidade por `last_frag`/`pending_fracks`).
- Janela `06 · MESH` só com `peers > 0`; drawer pelo marcador `MESH`.
- Box 03: CPU (RAPL) e GPU (NVML) separados, dois traços, `VENT` hwmon,
  `FAN +/−/HOLD/SEM SENSOR` (advisory, sem actuação), total J; não cresce
  (105 px no 720p, 132 px no 873).
- `iso-hw`: `uiMode=cinematic` (Chromium `--kiosk` fullscreen) + todos os GGUFs
  pinados (0.8B Q4/Q8, 4B Q4, 9B Q4) como symlinks content-addressed.

## Provas

- `VERIFY_OK` (fmt + clippy -D warnings + cargo test --workspace).
- 2 instâncias: A `:7700` N15 Master, B `:7702` N16 Worker; UDP 19000/19001;
  `peers=1`, `pinned=1`, `TOFU SETTLED`; screenshot
  `docs/evidence/artifacts/orb_ui_mesh_2nodes.jpeg`.
- Chromium 1283×873 e 1280×720: rails 591/591, box 03 105 px, mesh 06 live.
- `nix-instantiate --parse images/hw.nix`/`flake.nix`; `nix eval` do
  `uiMode` devolveu `cinematic`.
- `iso-hw` build: ver `docs/evidence/ORB_UI_0.7.0-alpha.1.md` (a preencher).

## Lições novas

1. **Opacity animada em painel** no Chromium/cage prende o layer a ~20%
   (texto/fundo escuros) apesar de `opacity:1` computado. Animar só
   `transform`; remover `#app.ready .panel { animation: ... opacity }`.
2. **MCP eco-tokens**: o Python não vê `git` no PATH; `git_diff_hash()` dá
   SHA256("") e o breaker bloqueia mesmo com mudanças. Reset é decisão humana
   (`.eco/state.json`/`update_task_status`), nunca `git checkout`.
3. **Windows mtime**: alterações no HTML podem ficar com mtime anterior ao
   binário; `cargo build` diz "Finished" sem recompilar. `cargo clean -p nosd`
   (e `nos-observe` quando aplicável) antes de testar o HUD.
4. **Duas instâncias nosd**: portas UDP distintas + unicast loopback,
   `--node-id` 15/16, data-dirs separados; `private`/TOFU fica pinned em
   segundos. Matar a B esconde a 06 no poll seguinte.
5. **Bootstrap/HTML**: o `include_str!` só é actualizado no binário; após
   editar `orb_lab.html`, rebuild/restart antes de diagnosticar UI.
6. **Screenshot**: o tool exige janela visível; a captura é a prova visual
   (não guardar prints pretos do screendump do virtio-gpu).

## Residual

- Metal/QEMU com câmara/mic/Piper reais AWAITING_HW; RAPL/hwmon só no Linux.
- Slint HUD nativo não foi reescrito.
- ISO `iso-hw` com todos os modelos: build de ~10 GB pendente/registado na
  evidência; testar firstboot + brain.plan/accept no metal.
- `mesh_node_id` no `health` pode vir `null`; os pins/roles do `health.mesh_nodes`
  são a fonte do HUD.
