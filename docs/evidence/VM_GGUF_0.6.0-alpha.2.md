# Evidence — VM GGUF / llama-server (0.6.0-alpha.2)

**Data:** 2026-10-27  
**Check:** `checks.x86_64-linux.nixaios-vm`  
**Modelo:** `stories15M-q4_0.gguf` (fixture `fetchurl`; não entra na ISO)  
**SHA-256 do fixture:** `66967fbece6dbe97886593fdbb73589584927e29119ec31f08090732d1861739`  
**Runtime:** NixOS VM (QEMU), CPU, sem GPU.

## Comando

```bash
nix build .#checks.x86_64-linux.nixaios-vm -L
```

## Resultado

```
llama-server spawned pid=552 base=http://127.0.0.1:... model=/var/lib/nosd/models/stories15M-q4_0.gguf ngl=0 api_key=set
hotswap attach llama-server-http
engine=llama-server-http
infer engine=llama-server-http latency_ms=38222 ok=true
hotswap unload -> cpu-echo (was llama-server-http)
llama-server spawned pid=940 ... api_key=set
hotswap attach llama-server-http
health | grep llama-server-http -> PASS
```

## O que prova

- `systemd.services.nosd.path` funciona (R-01).
- Landlock `Verified` **não** impede exec/read de `/nix/store` para subprocessos.
- `llama-server` spawn, `/health`, API key, `chat`, `unload` e reload.
- ModelHub regista o GGUF; `health.engine` reflete `llama-server-http`.

## O que não prova

- Performance/SLO (38s CPU/TGC no tiny model).
- Modelo 7B/13B, CUDA/ROCm, VRAM/thermal.
- ISO com modelo (política S1.2: sem seed; runtime fetch/HITL).
- Voz, cage/DRM, Wi-Fi, metal.
