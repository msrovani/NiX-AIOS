# NixAIOS 0.6.0-alpha.1 — Engine persistent

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.1  
**Data:** 2026-10-26  
**Tag:** `v0.6.0-alpha.1`  
**Base:** 0.5.6 (QEMU green)

## O que muda

- **E-01:** `llama-server` persistente é o default quando há um GGUF e o binário
  está disponível (`NOSD_LLAMA_SERVER_BIN` ou PATH). O daemon escolhe porta livre
  (ou `NOSD_LLAMA_SERVER_PORT`), espera `/health` até 30s, e mantém o filho vivo
  até unload/drop. Sem binário/timeout → fallback `llama-cli`; sem modelo →
  `cpu-echo`.
- **E-02:** inferência não segura o lock do HotSwap durante IO; `Arc<dyn
  InferenceBackend>` permite swap seguro e concorrência; log `latency_ms`.
- **E-04:** GGUF validado (magic/versão/contadores) antes de attach; ficheiro
  inválido é rejeitado.
- **M-01:** `seg.bin` faz `sync_data` e rejeita registo > 16 MiB (sem
  `vec![0; 4GiB]` em corrupção).
- **J-01:** `scripts/accept-m3.sh` cobre health/route/chat/recall/voice/ui.card.

## Evidência

```bash
cd packages/nosd
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

- `nix build .#nosd` PASS.
- `nix build .#checks.x86_64-linux.nixaios-vm` PASS (health + chat cpu-echo).
- QEMU 0.5.6 evidence permanece válida para o boot; engine real com GGUF/llama-server
  ainda **não** foi exercitado em VM/QEMU com modelo.

## Residual honesto

- Chat GGUF/llama-server e2e, voz, cage/DRM e Wi-Fi: não exercitados nesta fatia.
- S-04 completo (remover `hitl_confirmed` do admin), S-08/S-09.
- R-03/R-04 ring Nix desarmado; M4/M5 AWAITING_HW.
- `llama-server` spawn em systemd precisa de validação no ISO (PATH/unidade dGPU).
