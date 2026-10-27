# NixAIOS 0.6.0-alpha.2 — GGUF real no VM

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.2  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.2`  
**Base:** 0.6.0-alpha.1

## Correções críticas

1. **R-01 estava inerte:** `path` foi colocado dentro de `serviceConfig`; systemd
   ignorou com `Unknown key 'path'`. Movido para
   `systemd.services.nosd.path` (e helpers root). Isto explica `llama-server
   bin`/`llama-cli`/`nmcli` não encontrados.
2. **Landlock bloqueava subprocessos:** com `Verified`, o allowlist não incluía
   `/nix/store`/`/etc`/`/run`; executar `llama-server`, `nmcli` ou `systemctl`
   falhava por leitura negada. Adicionadas regras **RO** para
   `/nix/store`, `/run/current-system`, `/etc`, `/usr`, `/lib`, `/lib64`,
   `/bin`, `/sbin`, `/run`, `/var/log` e `/dev`.
3. **llama-server exposto:** spawn agora usa `--api-key` aleatória
   (nunca logada) + `--no-webui`; health e infer usam
   `Authorization: Bearer`.

## Evidência VM (GGUF real)

`checks.x86_64-linux.nixaios-vm` instala `stories15M-q4_0.gguf`
(`fetchurl`, fixture de teste — **não** entra na ISO) e executa:

```
nosctl health                                  -> ok
nosctl health | grep llama-server-http         -> ok
nosctl chat 'Once upon a time'                 -> text
nosctl unload                                  -> cpu-echo
nosctl load /var/lib/nosd/models/stories15M-q4_0.gguf
nosctl health | grep llama-server-http         -> ok
```

Logs reais:

```
llama-server spawned ... ngl=0 api_key=set
hotswap attach llama-server-http
engine=llama-server-http
infer engine=llama-server-http latency_ms=38222 ok=true
hotswap unload -> cpu-echo (was llama-server-http)
llama-server spawned ... (reload)
```

A latência ~38s é CPU/TGC do VM com tiny model; **não** é SLO de produto.

## Residual

- Metal/CUDA/ROCm real; p95/p99 de 7B; thermal/VRAM.
- Voz real, cage/DRM, Wi-Fi físico.
- S-04 completo (legado admin), S-08/S-09.
- R-03/R-04 ring; M4/M5.
- ISO 0.6.0-alpha.2 reconstruída e QEMU gate PASS:
  [`evidence/QEMU_0.6.0-alpha.2.md`](evidence/QEMU_0.6.0-alpha.2.md)
  (SHA-256 `6f9f2906501bd8cca826297d7e9f7ee4a97ea373d7f4ec42f84c4ab9c2f0d812`).
