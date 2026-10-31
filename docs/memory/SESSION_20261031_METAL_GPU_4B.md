# Sessão 2026-10-31 — metal GPU + iso-hw-cuda-4b (0.7.0-alpha.3)

## O que aconteceu

1. Logs do pendrive `NIXAIOSLOG` confirmaram: smi coredump, Chromium→Slint,
   S1 NGL=0, rota SOUL→S2Deep, capacity só `card1`.
2. Plano de correções em 4 fases; implementação + check ELF PASS.
3. Pedido: ISO HW só com Qwen3.5 4B → `iso-hw-cuda-4b` build + pack img.

## Lições

- patchelf + glibc 2.44: PT_LOAD do `.dynamic` precisa `PF_W` (OR bit 2).
- WSL xorriso em ISO ~7 GB + ~8 GiB RAM: Bus error possível; retry
  `max-jobs 1` / `cores 2` após `wsl --shutdown`.
- flake git tree: `git add` ficheiros novos antes de `nix build`.
- CRLF em `.sh` / `.nix` no Windows → `perl -pi -e 's/\r\n/\n/g'`.
- `active.gguf` no slot_map evita "4b" → GeneratorPro com ISO single-model.

## Artefactos

Ver `docs/evidence/METAL_GPU_4B_0.7.0-alpha.3.md`.
