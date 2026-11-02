# Mesh unicast seeds — evidência host (0.7.0-alpha.5 in-tree)

**Data:** 2026-11-02 (03:00 UTC)  
**Âmbito:** `--mesh-peer` / `NOSD_MESH_PEERS` / `services.nosd.meshPeers`
(ADR [`0118-mesh-unicast-seeds`](../adr-addenda/0118-mesh-unicast-seeds.md)).  
**Tree:** `a0b239d` + tranche alpha.5 (staged, sem commit) + diff de mesh ANVIL
(sem commit). Ver `forum.ndjson` ids `ANVIL-0002..0014`.

## O que foi provado

Duas instâncias reais do binário `nosd` (não unit test), no mesmo host, com o
**broadcast apontado a uma porta fechada** (`127.0.0.1:19999`). O único caminho
possível para o HB é a semente unicast.

| Nó | IPC | Mesh bind | Broadcast | Semente |
|----|-----|-----------|-----------|---------|
| A | `127.0.0.1:7800` | `0.0.0.0:19010` | `127.0.0.1:19999` (fechada) | `127.0.0.1:19011` |
| B | `127.0.0.1:7801` | `0.0.0.0:19011` | `127.0.0.1:19999` (fechada) | `127.0.0.1:19010` |

Log de A (8 s após o arranque):

```
mesh udp bind=0.0.0.0:19010 broadcast=127.0.0.1:19999 targets=2 peers=1
mesh targets: broadcast=127.0.0.1:19999 seeds=1 total=2
mesh HB peer=16 pk=true
mesh LayaIntent peer=16 arm=1 fnv=e8130093679f47c9
```

`health` de A:

```
mesh_peers = 1
mesh_seeds = ['127.0.0.1:19011']
machines   = 2   tofu_settled = True
peers      = [(16, True, 'Master')]
```

Artefactos: [`artifacts/mesh-unicast-20261102/`](artifacts/mesh-unicast-20261102/).
(Antes da normalização CLI→env, `mesh_seeds` aparecia `[]` apesar de
`targets=2`; o log da primeira corrida está em `forum.ndjson` `ANVIL-0006` e o
fix ficou em `main.rs`.)

## NixOS (avaliação, não build)

`nixosConfigurations.nixaios-iso.extendModules` com duas sementes
(`192.168.1.50:19000`, `10.0.0.9:19000`) avalia:

- `ExecStart` → `… --mesh-broadcast 255.255.255.255:19000 --mesh-peer
  192.168.1.50:19000 --mesh-peer 10.0.0.9:19000`
- `Environment` → `NOSD_MESH_PEERS=192.168.1.50:19000,10.0.0.9:19000`

Sem sementes, o `ExecStart` avaliado é igual ao anterior (nenhuma flag nova).
Isto prova o módulo/opção; **não** prova build da unidade nem arranque systemd.

## Regressão broadcast puro (lista vazia)

Sem `--mesh-peer`, o arranque mostra `targets=1` (só broadcast), provando que a
lista vazia não acrescenta destinos:

```
mesh udp bind=0.0.0.0:19012 broadcast=127.0.0.1:19013 targets=1 peers=0
mesh targets: broadcast=127.0.0.1:19013 seeds=0 total=1
```

## Duplicado por dois caminhos (identidade M6)

Com `NOSD_MESH_REQUIRE_SIG=1`, o mesmo HB chega duas vezes quando broadcast e
semente apontam ao mesmo socket (loopback `127.0.0.2` e `127.0.0.1`). O
`Pinned::check` passou a distinguir `Duplicate` (mesmo `seq`) de `Replay`
(`seq` antigo): 10 duplicados em 10 s ficam em `debug`, **zero** warns
"rejeitado (Replay)"; `mesh_peers=1`, `mesh_pinned=1`, `pk_bound=true`.
Sem esta distinção, cada HB duplicado gerava um WARN a cada tick (journal
spam que escondia replays reais).

## Testes automatizados

- `cargo test --workspace` → **rc=0**, 38 suites, **943 passed**, 0 failed,
  0 ignored, 0 panics.
- Teste novo: `unicast_seed_delivers_hb_without_broadcast` (loopback A/B,
  broadcast numa porta fechada) + 3 testes de `destinations()` (dedupe, lixo
  rejeitado, literal IPv6).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` →
  **rc=0** (depois de fechar 7 lints pré-existentes da tranche alpha.5; ver
  `ANVIL-0003..0005`).
- Repetição (Linux): 3× `cargo test -p nos-mesh` → 57 passed, 0 failed,
  8.71 s por corrida.
- Windows MSVC: `cargo test -p nos-mesh` → 57 passed, 0 failed (inclui o teste
  de semente unicast).

## Reproduzir

Com o binário construído (`cargo build -p nosd --bins`):

```
NOSD_BIN=…/nosd NOSCTL_BIN=…/nosctl bash scripts/smoke-mesh-unicast.sh
# SMOKE_MESH_OK targets=2 peers=1 seed=127.0.0.1:19111
```

O smoke usa portas próprias (17800/17801, 19110/19111, broadcast morto em
19199), falha com rc=1 se o HB não chegar pela semente ou se o `health` não
mostrar `mesh_peers=1`/seed.

## Tested

- Parse/dedupe de destinos; rejeição fail-closed de semente inválida.
- Entrega do HB por semente unicast sem broadcast; `pk_bound`/TOFU settle.
- `health.mesh_seeds` reflete CLI e env; `config.show` idem.
- Clippy/test workspace verdes com o diff aplicado (host WSL, Rust 1.99.1).

## Not-tested

- Build da ISO/unidade: o eval de `nixaios-iso.extendModules` passou (ExecStart
  e Environment corretos), mas a derivação da unidade e a ISO não foram
  construídas.
- QEMU `mesh-2vm`/`mesh-3vm-faults` e metal (Core 7 / i5 / 7700G).
- Wi‑Fi com AP isolation real, NAT, segmentos roteados, IPv6 link-local.
- FRAG/FRACK por semente (o código envia para todos os destinos; não foi
  exercitado ponta-a-ponta).
- Regressão de broadcast puro (lista vazia) além dos testes existentes.
