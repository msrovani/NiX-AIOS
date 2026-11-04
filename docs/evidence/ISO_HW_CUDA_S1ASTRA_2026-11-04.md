# ISO iso-hw-cuda — S1 Astra (0.8B Q4_K_M) + S2 4B Q6_K (2026-11-04)

Build `.#iso-hw-cuda` — primeira ISO com o System 1 = Qwen3.5-0.8B
**editado pela Astra** (Weight-Space Programming) e System 2/PRO =
Qwen3.5-4B **Q6_K** na dGPU (`llama-server :8082`, `--fit on --fit-ctx
2048 --reasoning off`).

## Artefacto

| Campo | Valor |
|---|---|
| Atributo | `.#iso-hw-cuda` (flake @ `a3ee3e0`) |
| ISO | `/nix/store/jwrkq1fnf7wccdgv7r0l86hcdch1nr0r-…/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso` |
| Tamanho | 8 323 596 288 B (7.75 GiB) |
| sha256 | `c2da2cfd2a97d591f9dc2bf4af660f2a1212a4fead885063a90656c7d7a80195` |

## Prova dos GGUFs embutidos (unsquashfs de `/nix-store.squashfs`)

- S1: `8wv2737hqv8wgxmmq2mkypymzav0sspj-Qwen3.5-0.8B-Weight-Space-Programming-Q4_K_M.gguf`
  — **mesmo store path hash-verified** do flake `qwen35-08b-q4km`
  (LFS sha `0be511c8…`, edito Astra bit-exact).
- S2: `0cpj2dkknhvwnzw8lj6qap5mib2mkdv8-Qwen3.5-4B-Q6_K.gguf`
  — == `nix eval --raw .#packages.x86_64-linux.qwen35-4b-q6k`
  (unsloth Q6_K, SRI `/e3XgcnOZ2q2…`). **Sem edito** (o edito da Astra
  é só no 0.8B).
- STT: `3hxqscirmb90103r7v7lqhygaiqjlm1g-ggml-tiny.bin`; TTS:
  piper-standalone + espeak (Diogo/Andrea/…, pt_BR-faber onnx).

## Packing (pendrive único)

- Artefacto flash: `~/nixaios-hw-cuda-s1astra-20261104.img` (WSL ext4,
  `/home/msrov/`), 12 618 563 584 B (11,75 GiB),
  sha256 `dd15109d18029433cd12e0d1565f7b4174a8fabd9d9cf45260a3f23917e6eb2e`.
- Partição 3 = FAT32 4 GiB montada por losetup e confirmada
  (`FAT_VERIFIED`); ISO na part 1/2 como já é do costume.
- A FAT nasce vazia a dado (`README` descreve seed opcional de
  `models/active-…gguf` e `wifi.conf` e respostas do firstboot);
  os GGUFs de S1/S2 viajam **embutidos** no squashfs da ISO.

## Tested / Not-tested

- **Tested**: `nix build .#iso-hw-cuda` (out-path + closure); sha256 do
  ficheiro; presença dos dois GGUFs por nome+hash dentro do squashfs da
  ISO; store-path do S2 == pin do flake.
- **Not-tested**: metal com este S1 editado (driver/CUDA/VRAM/voice);
  edito sobreviver à quantização Q4_K_M em produção (ver
  `docs/evidence/S1_ASTRA_EDIT_2026-11-04.md`); boot gate
  (`--stage MQE`) da ISO.

## HUD C1–C6 (corrige o que os fotos de metal mostraram)

Análise das fotos HUD de metal (2026-11-02/03) levou ao plano C1–C7,
implementado no commit `2fa3e61` (apresentação/derivação; sem novos RPCs
com side-effect, HITL intocado):

- **C1** REDE: estado derivado do link real (`NetLinkState`
  `associated|no-link|down` em `nos-observe`); iff-up administrativo sem
  SSID/carrier deixa de pintar UP (era o P1 dos fotos).
- **C2** ENERGIA: badge de scope `CPU|GPU|FULL|EST|—`; `MED` só-GPU
  silencioso morto (era o P2).
- **C3** VOZ: painel 02e separado (deixa de estar aninhado em REDE, P3),
  `data-optional` (esconde no ≤800px, disponível no drawer DIAG).
- **C4** DÍVIDA: linha SIMBIOSE clicável → drawer com lições abertas
  (id/topic/text/why + `hint` derivado de `Pre::holds` — `nix.audit` +
  `orb.symbiosis` read-only; matava o P4).
- **C5** SUSPEND: texto completo `PROPOR SUSPEND` no painel VOZ com
  etiqueta `TIER HITL` (P5); o grant continua server-authored.
- **C6** PROPORÇÃO: rail esquerda 25vw (`minmax(300px,25vw)` + kiosk
  queries ≤800px/≤1200px); prova headless-Chromium 720p em
  `docs/evidence/artifacts/orb_hud_25vw_720p_layout.png` (~25.1%, sem
  truncamentos P6/P7).
- **C7** evidência: este documento + artefactos `orb_hud_25vw_720p_*.png`.

**Not-tested dos fixes**: HUD com os fixes em QEMU/metal (ISO re-build
id-08 abaixo); LinkState/energy badge contra os hardware reais.

## Rebuild pós-fixes (PENDING)

- `nix build .#iso-hw-cuda` com as mudanças HUD + `pack-hw-usb.sh`:
  **PENDENTE** (nova ISO/`.img`, novos sha256 a actualizar neste doc).
