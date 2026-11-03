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

## Tested / Not-tested

- **Tested**: `nix build .#iso-hw-cuda` (out-path + closure); sha256 do
  ficheiro; presença dos dois GGUFs por nome+hash dentro do squashfs da
  ISO; store-path do S2 == pin do flake.
- **Not-tested**: metal com este S1 editado (driver/CUDA/VRAM/voice);
  edito sobreviver à quantização Q4_K_M em produção (ver
  `docs/evidence/S1_ASTRA_EDIT_2026-11-04.md`); boot gate
  (`--stage MQE`) da ISO; `pack-hw-usb.sh` não corrido (o `.img` único
  com FAT `NIXAIOSLOG` é passo seguinte antes de DD).
