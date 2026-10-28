# NixAIOS 0.6.0-alpha.11 — Frota 3-VM com falhas (e dois bugs corrigidos)

**Volid ISO:** `NIXAIOS051`
**Workspace:** `packages/nosd` 0.6.0-alpha.11
**Data:** 2026-10-27
**Tag:** `v0.6.0-alpha.11`
**Base:** 0.6.0-alpha.10

## O que entra

O objetivo desta release e um so: **provar a frota com falhas** (3 nos) e
corrigir o que essa prova revelou. Nao ha feature nova.

### Check `mesh-3vm-faults`

3 VMs na mesma VLAN, HB assinado + TOFU, jobs em Noise:

- convergencia de estado nos tres nos (A -> B -> C);
- **latencia** de 200 ms (`tc netem delay`) sem quebrar um job;
- **perda total** (`netem loss 100%`): A->C falha, A<->B continua;
- **`kill -9`** no nosd de C: `Restart=always` levanta-o e o log `NSGC`
  devolve o estado escrito antes da morte;
- **particao** (`ip link set eth1 down`) e recuperacao;
- convergencia final de todas as chaves nos tres nos.

### Bugs reais encontrados e corrigidos

1. **Delta do CRDT (M8.1):** o `vv[peer]` era incrementado **por chave**. A 2a
   chave nova de um no nascia com o mesmo `vv` da 1a e ficava fora de
   `delta_since`: `crdt.sync` respondia `ok:true, sent:0` e o peer **nunca** a
   recebia. Fix: contador global por peer. Teste:
   `second_key_of_same_writer_is_in_delta`.
2. **HB depois de restart (M6):** o `hb_seq` comecava em 1 em cada processo; os
   peers consideravam o HB do no reiniciado um **replay** e rejeitavam-no para
   sempre (metadados velhos: capacidade/role/liveness). Fix: base em epoch ms.
   Teste: `hb_seq_is_wall_clock_based_across_restart`; o check passou a assertar
   **0** rejeicoes apos o kill.

### CI

- O workflow e o `scripts/ci-local.sh` passam a incluir o **6o check**
  (`mesh-3vm-faults`), continuando a frota sequencial no espelho local.

## ISO

`nix build .#iso` ->
`/nix/store/49ym2x1sbh52vkddqdkxjpgmj7wwfi7k-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso` ->
SHA-256 `4a6f2957785f8802f910304a011bc692de878a8afad77f0a6c5b5074bd0fb643`.
Gate `MQEMU` **PASS** (`fails=0`, QEMU/TCG OBSERVED). Detalhe e serial:
`docs/evidence/QEMU_0.6.0-alpha.11.md`.

## Evidencia

| Item | Artefacto |
|---|---|
| Frota 3-VM com falhas | `docs/evidence/M5.1_3VM_FAULTS_0.6.0-alpha.10.md` |
| Log filtrado do check | `docs/evidence/artifacts/mesh-3vm-faults-0.6.0-alpha.10.txt` |
| ISO/QEMU | `docs/evidence/QEMU_0.6.0-alpha.11.md` |
| Fix VV do CRDT | `second_key_of_same_writer_is_in_delta` (`nos-mesh::crdt`) |
| Fix seq do HB | `hb_seq_is_wall_clock_based_across_restart` (`nos-mesh`) |

## Residuais (declarados)

- **3 VMs num so host** (QEMU, VLAN virtual): nao e multi-host, NAT nem Wi-Fi;
  nao substitui metal.
- **Kill -9 reinicia o processo**, nao a VM: prova replay do log, nao perda de
  energia.
- **Sem TTL/eviccao de peers**: a particao nao faz `mesh_pinned` descer;
  liveness/capacidade de um peer isolado fica stale.
- **netem** so com `delay 200ms` e `loss 100%`; sem jitter/reorder.
- **HB continua em claro** (assinado, mas legivel na LAN).
- **CI no GitHub**: primeira execucao externa continua pendente; o espelho
  local cobre os 6 checks.
- **nsgdb** continua a ser segmento local (sem indice/BQ); **M4/M5 metal**
  `AWAITING_HW`. QEMU ≠ metal.

## Demo manual 3-VM na ISO (o que ficou provado e o que nao)

Tentativa de correr **a propria ISO** em 3 QEMU com video (VNC/GTK) e ligar a
mesh entre eles. Resultado honesto:

| Item | Estado |
|---|---|
| 3 VMs com HUD/JARBAS e virtio-gpu | **OBSERVED** (janelas e screenshots) |
| IP estatico em `ens3` + rotas de broadcast nos 3 | **OBSERVED** |
| `mesh_pinned: 2` na ISO | **NAO** ? o restart do nosd para aplicar o drop-in (`REQUIRE_SIG` + `JOB_BIND`) nao teve efeito (`hermes_ticks` mostrou que o daemon nunca reiniciou) |
| Prova da mesh | **`mesh-3vm-faults`** (3 QEMU, PASS) ? nao a demo da ISO |

Notas de infraestrutura descobertas (uteis para uma demo futura):

- `-netdev socket,listen` do QEMU aceita **uma** ligacao; para N VMs e preciso
  um hub. Neste WSL, `mcast` nao entregou e nao ha `sudo` sem password para
  bridge/TAP; a solucao foi um **relay UDP em userspace** (1 datagrama = 1
  frame), que recebeu 297 frames e reenviou 594.
- A ISO usa o **firewall NixOS default** (ao contrario dos checks, que o
  desligam): sem abrir UDP 19000/TCP 19001, o HB morre no INPUT.
- Sem rota, `sendto(255.255.255.255)` da `ENETUNREACH` (visto no journal).

### Bug encontrado (alpha.12)

`modules/transmute/firstboot.sh` acrescenta `--no-download` **duas vezes**
(modo auto **e** offline), e o `nosctl transmute` recusa com
`the argument '--no-download' cannot be used multiple times`. A imagem cai
para a GUI na mesma, mas o passo de transmute offline falha com `exit=2`.
