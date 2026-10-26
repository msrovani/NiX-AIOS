# Lab mesh 2-nó + fleet HUD (sketch)

**Honesty:** roteiro manual apenas — **não** verificado e2e neste repo.

## Objetivo

Dois `nosd` com UDP mesh ligado; `fleet.health` / `health` RPC expõem `fleet_worst`;
EventBus `FLEET_HEALTH` alimenta orb (agitação em `no_go`).

## Pré-requisitos

- Build: `cargo build -p nosd --release` (target-dir isolado ok).
- Portas distintas: nó A `7700`, nó B `7701`.
- Mesh: `NOSD_MESH_BIND=0.0.0.0:19000` nó A, `19001` nó B; broadcast `255.255.255.255:19000` (ajustar conforme rede).

## Passos (Windows/WSL ou NixOS)

1. Terminal A:
   ```powershell
   $env:NOSD_MESH_BIND="0.0.0.0:19000"
   $env:NOSD_MESH_BROADCAST="127.0.0.1:19000"
   cargo run -p nosd -- --node-id 10 --listen 127.0.0.1:7700
   ```
2. Terminal B:
   ```powershell
   $env:NOSD_MESH_BIND="0.0.0.0:19001"
   $env:NOSD_MESH_BROADCAST="127.0.0.1:19000"
   cargo run -p nosd -- --node-id 20 --listen 127.0.0.1:7701
   ```
3. RPC (nosctl ou curl lab):
   ```powershell
   cargo run -p nosd --bin nosctl -- health
   ```
   Esperado quando peers>0: campos `fleet_verdict`, `fleet_machines`; método `fleet.health`.

## Gaps conhecidos

- MCH **não** é enviado automaticamente no heartbeat (só parse RX).
- CRDT / mesh_knowledge merge: **NOT_DONE**.
- LayaIntent wire: encode/decode unit-tested; produtor runtime **NOT_DONE**.
