# Sessão 2026-11-04 — HUD C1–C8 + rebuild pendrive (FAT 1 GiB)

## Contexto

Fotos do metal 0.7.0-alpha.5 (Core 7 240H) mostraram defeitos no HUD
`orb_lab.html`: rede "ligada" com iff-down, energia "MED" sem scope,
VOZ espremida dentro do REDE, botão suspend truncado, coluna esquerda
não-1/4. Plano C1–C8 aprovado pelo usuário.

## Decisões

- Só apresentação/derivação: **sem RPC novo com side-effect**, HITL
  intocado (grants server-authored).
- Rebuild do pendrive **mantendo voz** (STT ggml-tiny + piper/espeak);
  opção "só os 2 Qwen" recusada (exigiria atributo novo no flake).
- FAT `NIXAIOSLOG` reduzida **4096 → 1024 MiB** (`FAT_MB=1024` no
  `pack-hw-usb.sh`) — "1 GiB legível no Win11".
- Versão **0.7.0-alpha.6** (tranche in-tree HUD; ISO do rebuild é a
  `iso-hw-cuda` corrente, não release nova).

## Entregue

- Código: `2fa3e61` — `nos-observe::NetLinkState`/`link_state()`,
  `renderNet/renderEnergy/02e VOZ/panelSwitch/openLessonsDrawer`,
  `hint_for()` em `nos-core::nix_domain`, `needless_return` actuator.
- Artefactos: `01fd4f4` — screenshots 720p headless-Chromium
  (`docs/evidence/artifacts/orb_hud_25vw_720p_*.png`).
- Docs: `e60e930` + `9e3bfd3` — §HUD C1–C6 + tabela rebuild no
  `docs/evidence/ISO_HW_CUDA_S1ASTRA_2026-11-04.md`.
- Pentdrive: ISO sha `5747c666…8a41`; `.img` 9 397 338 112 B sha
  `09c4e76f…f194`; FAT32 p3 start 16 257 024, label `NIXAIOSLOG`,
  mount ro OK (`wsl -u root`); squashfs: S1 Astra `8wv2737h…`,
  S2 `0cpj2dkk…`, ggml-tiny + piper (409 G-matches).

## Aprendizado operacional (→ AGENTS.md lições 50–52)

- FAT em `.img` híbrido: mtools offset (`mdir -i img@@N`) mente com
  `non DOS media` — usar `losetup -P` + mount ro em `wsl -u root`; e
  `losetup -d` no fim (loop ficou pendurado de ronda anterior).
- Job WSL da sessão morreu sem rasto e levou o log de `/tmp`: tarefa
  longa = script bash + log em `$HOME` (não `/tmp`).
- "A ISO contém o modelo X" só se prova com extract squashfs +
  `unsquashfs -l | grep` com contagem, comparado com os pins do flake.

## Not tested

- Boot metal no pendrive novo (AWAITING_HW); escrita guest na FAT 1 GiB;
  leitura real no Win11 (Disk Management Online não re-observado).
