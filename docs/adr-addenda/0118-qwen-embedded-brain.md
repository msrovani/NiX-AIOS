# ADR-0118 addenda — Brain embutido (Qwen3.5-0.8B) na iso-hw + upgrade proposto pós-boot

**Estado:** ACEITA (2026-10-28). Afeta: `iso-hw` (metal, NUNCA release), `nosd`
(model.plan / blob.fetch), `jarbas` (card HITL), `flake.nix` (packages `qwen35-08b-*`).

## Contexto

A regra S1.2 ("sem seed de blobs na ISO") existia para: (1) manter a ISO
reprodutível, (2) obrigar fetch deliberado com HITL, (3) ficar magra. O metal
real criou um nível de aceitação novo (`iso-hw`, FAT32 `NIXAIOSLOG`): a
primeira experiência tem de **funcionar sem rede** — baixar o brain no
first boot criava outra cadeia de falhas (download → grant → hot_swap → HUD),
tal como a lição pós-Q5 de EACCES.

## Decisão

1. **iso-hw embute o brain:** `Qwen3.5-0.8B GGUF Q4_K_M` (unsloth,
   Apache-2.0, ~508 MiB, pin SRI
   `sha256-vSWHguNff0WPis7RrcBT5ukuibxzW6O+idOKBhIdxRc=`,
   size 532.517.120 B) como **derivação Nix** (`flake.nix`:
   `packages.qwen35-08b-q4km`), montada/apontada em `models` pelo módulo
   `iso-hw`. `llama.cpp` já vai incluído (llama-server).
   - **Nada de baixar no primeiro boot** — o env `NOSD_FIRSTBOOT_DOWNLOAD`
     fica OFF no iso-hw (o firstboot reference o GGUF que já está no store).
3. **A ISO de RELEASE continua limpa** (S1.2 mantida para release; runtime
   fetch + HITL, como hoje).
4. **Reprodutibilidade:** embutir via derivação content-addressed é um
   *upgrade* de honestidade sobre o download runtime (que vinha de URL
   sem pin): SRI + tamanho fixados no flake; proveniência no ADR.
5. **Upgrade pós-boot é PROPOSTA, não automático:** se o hw permitir algo
   maior/melhor/melhor quantização, o **JARBAS propõe** — card HITL real
   (`ui.action` → grant server-authored kind `blob.fetch` / `nix.apply`
   para troca declarada). Nunca auto-switch.

## Tiers de proposta (capacidade → candidatos)

| Posture (capacity prober) | Candidato proposto | Modo |
|---|---|---|
| ≤2 GiB RAM avail / CPU fraca | **Qwen3.5-0.8B Q4_K_M** (embutido, default) | `llama-server` spawn managed |
| RAM ≥4 GiB | Qwen3.5-0.8B **Q8_0** (`packages.qwen35-08b-q8`, ~774 MiB) | spawn managed |
| RAM ≥8 GiB / iGPU | Qwen3.5-2B/4B GGUF Q4 (fetch runtime + HITL `blob.fetch`) | spawn managed |
| dGPU (VRAM ≥6 GiB) | Qwen3.5-4B/9B Q4 com `-ngl` (plano de `plan_n_gpu_layers`) | `nosdEngine` dGPU / CUDA VERIFIED |

Regras: a proposta usa o posture **observado** (nunca fake); cancel/expired
refazem a proposta a cada boot; `cpu-echo` continua o fallback honesto.
mmproj (visão, ~205 MiB F16) fica para tier próprio — NÃO embutir agora.

## Consequências / contratos

- `transmute --no-download` no iso-hw continua a escrever o
  `host-configuration.nix`; o hot swap segue imediato (cage → HUD).
- O grant `blob.fetch` (S-04) deixa de estar no caminho crítico do
  primeiro boot no metal e passa a estar no **caminho de upgrade** —
  onde é mais valioso: só hoje baixa quem aceita a proposta.
- `fetch_to_dir` (cap 8 GiB, selo de ficheiro mágico, rename atómico)
  continua o runtime da proposta até G4.1 (modelo como derivação
  substituível numa config) fechar o ciclo.
- **NÃO** é uma reversão da S1.2 para release — é uma excepção escopada a
  `iso-hw`, documentada com hash; o lint `guard-iso-no-s1-blob.sh`
  continua a apontar **`iso`**, não `iso-hw`.

## Verificação (o fecha o aceite)

1. `nix build .#qwen35-08b-q4km` (host Linux) → hash SRI bate.
2. iso-hw em QEMU: boot → sem pedido de rede →
   `nosctl health` = `llama-server-http` com Qwen3.5-0.8B →
   `nosctl chat "oi"` p95 medido (meta ≤ 15 s CPU).
3. Proposta de upgrade: card HITL → aceitar → Q8_0/pesosFetch arrive em
   `/var/lib/nosd/models` → hot_load, com proveniência
   (`who/what/when/nonce`) no log.
4. Recusar a proposta: nada baixa; brain continua o embutido.
