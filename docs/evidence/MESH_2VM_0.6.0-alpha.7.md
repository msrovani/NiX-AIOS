# Evidence — Mesh 2-VM (M5 lab, 0.6.0-alpha.7)

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux` VM ×2 — **OBSERVED** (lab, não metal, não frota real)  
**Check:** `checks.x86_64-linux.mesh-2vm`  
**Binário:** `nosd` 0.6.0-alpha.7 (sem alteração de produto nesta entrega)

## Topologia

```
node a (id 11) 192.168.50.1/24 ── VLAN 1 (nixosTest) ── node b (id 12) 192.168.50.2/24
      nosd: UDP 0.0.0.0:19000, broadcast 192.168.50.255:19000, hermes tick 1s
```

## Asserções (todas PASS)

| Asserção | Comando |
|---|---|
| peer descoberto nos dois lados | `nosctl health \| grep -q '"mesh_peers": 1'` |
| HB com PK no wire (TOFU pk-bound) | `journalctl -u nosd -o cat \| grep -q 'pk=true'` |
| eleição stub (capacidade igual → menor id) | `health \| grep -q Master` (a) / `Worker` (b) |

Saída real de `nosctl health` no nó **a**:

```json
{
  "result": {
    "fleet_machines": 2,
    "mesh": "[{\"local\":11,\"peers\":1,\"tofu\":0,\"last_frag\":0,\"role\":\"Master\",\"pending_fracks\":0}]",
    "mesh_peers": 1,
    "landlock": "Verified",
    "hermes": "running",
    "engine": "cpu-echo"
  }
}
```

Log do daemon (ambos os lados, ~1 HB/s):

```
nosd: mesh HB peer=12 pk=true
nosd: mesh HB peer=11 pk=true
nosd: mesh LayaIntent peer=12 arm=1 fnv=e8130093679f47c9
```

O `fnv` do LayaIntent é **igual nos dois nós** (`e8130093679f47c9`) — o
intent propagado converge, mas isso é estado de intenção, não CRDT de knowledge.

## TOFU settle

O flag `tofu` no JSON é `tofu_settled()` (janela de `TOFU_SETTLE_MS` = 7.2 s ou
130 ticks) e estava `0` no snapshot (early). Coberto por unit test:

```
cargo test -p nos-mesh tofu
test tofu_settles_after_window ... ok   (7.50 s)
```

## Honestidade / residual

- **Frota real (M5) não está provada**: 2 VMs na mesma VLAN não exercitam
  Wi-Fi, perda de pacotes, NAT, relay, nem >2 nós.
- **CRDT wire sync é residual**: `fleet.health` diz explicitamente
  `"CRDT NOT_DONE"`. O mesh entrega HB, MCH, LayaIntent, FRAG/FRACK e
  eleição; a sincronização de knowledge/CRDT entre nós **não** é testada aqui.
- `tofu` no snapshot é 0 no arranque; o settle é provado por unit test, não
  pela VM (o campo vive dentro de uma string JSON escapada).
- QEMU/VM ≠ metal (M4 continua AWAITING_HW).
