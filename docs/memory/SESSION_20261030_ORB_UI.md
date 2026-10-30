# Sessão 2026-10-30 — ORB UI 0.7.0-alpha.1 → alpha.2

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
  pinados (0.8B Q4/Q8, 4B Q4/**Q6**, 9B Q4) como symlinks content-addressed.

## Provas

- `VERIFY_OK` (fmt + clippy -D warnings + cargo test --workspace).
- 2 instâncias: A `:7700` N15 Master, B `:7702` N16 Worker; UDP 19000/19001;
  `peers=1`, `pinned=1`, `TOFU SETTLED`; screenshot
  `docs/evidence/artifacts/orb_ui_mesh_2nodes.jpeg`.
- Chromium 1283×873 e 1280×720: rails 591/591, box 03 105 px, mesh 06 live.
- `nix-instantiate --parse images/hw.nix`/`flake.nix`; `nix eval` do
  `uiMode` devolveu `cinematic`.
- `iso-hw` build: ver `docs/evidence/ORB_UI_0.7.0-alpha.2.md`.

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
7. **CRLF em `flake.nix`**: `Path.write_text` no Windows grava `\r\n`; dentro
   de `postFixup = ''…''` isso vira `$'\r': command not found` no fim da
   fixupPhase. O build passa 10 min e falha no fixup. Converter o ficheiro
   para LF antes de `nix build` (o Nix usa a árvore dirty, não o blob git).

## Addendum 0.7.0-alpha.2 — 6 GB tier + contexto máximo

- **A linha Qwen3.5 não tem 7B** (small: 0.8B, 2B, 4B, 9B). O 9B Q4 não cabe
  em 6 GB: 5,68 GB só de pesos e `min_vram_mb=6600` no `brain_upgrade`.
- Decisão: **4B Q6_K** (`3 525 956 768 B`, sha256 `fdedd781…8f66`) como tier
  de 6 GB, escolhido **antes** do 4B Q4 quando `ram_avail_mb >= 4800`; o Q4
  continua fallback para máquinas pequenas.
- **Dois candidatos com o mesmo `tier=3`**: `accept(tier)` passou a resolver o
  candidato pelo **mesmo** `pick_reachable_tier`, senão aceitava sempre o
  primeiro da lista (Q6) mesmo sem headroom. Teste `low_ram_tier3_still_picks_q4`.
- **Contexto**: removido `NOSD_LLAMA_CTX=4096` da `iso-hw`; `llama-server`
  recebe `--fit on --fit-ctx <min>` (default 2048). Sem `-c` fixo, o fit
  escolhe o maior contexto que cabe; um `NOSD_LLAMA_CTX` explícito manda.
  `--help` do llama.cpp 0.6.0 confirma `-fit`/`-fitc` (default do fit-ctx é
  4096; usamos 2048 para não falhar em 6 GB).
- `iso-hw` alpha.2 leva **5 GGUFs**; os hashes alpha.1 não são reutilizáveis.
- Build final: ISO **15 946 315 776 B** sha256 `c5d7cb80…f67f`; `.img` único
  **20 241 711 104 B** sha256 `4662a340…9225`. O C: não tinha espaço para o
  ISO standalone depois do `.img` (12,9 GiB livres vs 14,85 GiB).

## Residual

- Metal/QEMU com câmara/mic/Piper reais AWAITING_HW; RAPL/hwmon só no Linux.
- Slint HUD nativo não foi reescrito.
- ISO `iso-hw` com todos os modelos (5 GGUFs, ~13 GB): rebuild alpha.2 e
  firstboot + brain.plan/accept no metal.
- `mesh_node_id` no `health` pode vir `null`; os pins/roles do `health.mesh_nodes`
  são a fonte do HUD.
