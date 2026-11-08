# Sessão 2026-11-06 — plano de inferência F1–F7, prova QEMU e o argv sem `-ngl`

Memória operacional da sessão que fechou o plano
[`docs/PLAN_INFERENCE_VRAM_VOZ_FABRICA.md`](../PLAN_INFERENCE_VRAM_VOZ_FABRICA.md)
e o levou até uma imagem real em QEMU. Commits: `8f08cbd` (F1), `d7b7884` (F2),
`575095a` (F3), `7a62319` (F4), `9ddfac7` (F5), `c3ce2a4` (F6), `d175b0c` (F7),
`b54a5f9` (docs F2–F7), `86e09b2` (evidência QEMU + lições).

## O que o plano pedia e o que ficou

| Fase | Defeito | Correção |
|---|---|---|
| F1 | `-ngl` explícito anulava o `--fit on` (OOM no metal de 3 Nov) | 4 sítios de `set_var("NOSD_LLAMA_NGL")` apagados; cap térmico em `AtomicU32` do processo, só para `-ngl` explícito; `SwapError::Boot` |
| F2 | `fit_model` aprovava modelos a cair na RAM como se fossem VRAM | `fit_model(need_vram, need_ram, ram, vram, allow_spill)` + `basis`; `need_estimate`; `hub.evict`; `vram_free_mb` por placa; `register_path` com `conflict` |
| F3 | status HTTP do llama-server perdia-se no `ureq` | `SwapError::Http{code,body}`; retry só 400/422 |
| F4 | TTS com voz de outro idioma; tiny preferido; dois backends no mesmo WAV | `piper_model_matches_lang`, base antes de tiny + `stt_model_reason`, um backend por WAV, `aplay -l` |
| F5 | fábrica chamava `cargo`/`python3` do PATH do unit (vazio) | `nix store ping` + `nix build --no-link --print-out-paths`; `write` recusa dir preenchido; `Lang::detect` por token |
| F6 | `max_pages` declarado e ignorado | `min(heap, páginas × 64 KiB)` |
| F7 | RAPL lia só `intel-rapl:0` | soma de todos os `intel-rapl:N` |

C3.2/C3.3 entraram na F1 de propósito: apagar os `set_var` sem consumidor real
deixaria apenas um log inerte.

## Gate (host, WSL, `nix develop`)

`fmt` limpo, `clippy --workspace --all-targets --all-features -D warnings` sem
avisos, `cargo test --workspace`: 1015 → **1021** → 1029 → 1031 → **1037**.

O gate apanhou três defeitos antes de cada commit: E0382 (`path` movido em
`register_path`), `let mut resp` sem mutação, e `split(|c| …)` que o clippy
queria como `split(['_','-'])`.

## A prova QEMU (o que valeu a pena)

Detalhe em [`docs/evidence/QEMU_INFERENCE_2026-11-06.md`](../evidence/QEMU_INFERENCE_2026-11-06.md).
Resumo do que **funcionou** e deve repetir-se:

1. `nix build .#iso-hw` com a árvore **limpa** (= `HEAD`) — sem `--impure`, o
   flake vê os commits, e a ISO fica com `sha256 dbe9d0e5…`, 7 050 903 552 B.
2. `run-iso-qemu-wsl.sh` deu `SKIP` no `nosd` do gate: o auto-login em `ttyS0`
   (`nixos login: nixos (auto)`) toma a consola antes do
   `Started Neural-OS Daemon`. **Não** se maquilha o log — lição 62.
3. Arranque alternativo com `-serial unix:$HOME/ser.sock,server,nowait
   -display none` e um cliente Python. Com ele: `is-enabled` → `enabled`,
   `is-active` → `active`, `status` com `since 22:53:07`, e o journal com
   `suggest_ngl=0 (hint only — --fit decides…)`,
   `hub slot=active conflict keep=… ignored=…`,
   `spawned … ngl=0 thermal_cap=None`.
4. argv de dentro do guest (`/proc/1221/cmdline`): sem `-ngl`, `--fit on`,
   `--fit-ctx 2048`, `--reasoning off`; **um** `llama-server`.
5. O 4B não cabe no guest (4 GiB vs 5,4 GB): a prova dele veio do binário novo
   **no host** com o GGUF real — `ngl_args=0`, `llama_server_procs=1`.

Três armadilhas que custaram tempo: TCG por falta de grupo `kvm` (boot ~6–10
min); `is-active` lido cedo devolveu `inactive` falso (o daemon subiu depois); e
`run-iso-qemu-wsl.sh` com `stdio` em `/dev/null` não deixa perguntar nada.

## O que ficou por provar (honestidade)

- **Metal** — nada; QEMU ≠ metal (ADR-0113).
- **Lane S2/CUDA com o 4B na GPU** — o guest não tem NVIDIA.
- **`jarbas-cage`** `inactive` no QEMU (sem GPU/VT); HUD não visto.
- **Voz** — nenhum WAV tocado.
- **C5.1** — `nix build` como `nosd` exige `nix-daemon` + `trusted-users`; sem
  isso cai no PATH antigo de propósito.
- **Retry do 400 (F3)** — provado na função pura, não com um stub HTTP.

## Para a próxima sessão

1. Fechar a cegueira do gate: aceitar `journalctl -b -u nosd` como segunda
   fonte quando o log de série perder a consola (lição 62) — é mudança no gate,
   com teste próprio.
2. Colheita metal Core 7 a correr `iso-hw-cuda` e a provar S1(CPU) + S2(CUDA)
   com uma só cópia do 4B e o argv sem `-ngl`.
3. Infra C5.1 (`trusted-users` para `nosd`) para a fábrica poder construir.
