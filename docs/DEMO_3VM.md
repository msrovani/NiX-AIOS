# Demo 3-VM (`iso-demo`) — mesh + vídeo

**Isto NÃO é uma ISO de release.** `iso-demo` existe só para demonstrar/testar a
mesh em 3 VMs com vídeo, sem a configuração manual por serial que a ISO live
exige. A ISO de release (`.#iso`) continua **sem identidade, sem firewall
aberto e sem job listener** por omissão.

## O que a imagem traz (e é inseguro de propósito)

- `services.openssh` **forçado ligado** (a ISO live desliga-o com `mkForce
  false`) com `root`/`nixaios` e password auth — apenas para o host fazer
  asserções.
- `networking.firewall.enable = false` (UDP 19000 / TCP 19001 abertos).
- `demo-net.service`: IP **derivado do MAC** da NIC de mesh
  (`52:54:00:12:34:0N` → `192.168.50.N/24`) e rota de broadcast.
- `nosd` com `NOSD_MESH_REQUIRE_SIG=1`, `NOSD_JOB_BIND=0.0.0.0:19001` e
  `meshBroadcast = 192.168.50.255:19000`; a identidade é criada no arranque.

## Como correr

```bash
nix build .#iso-demo
out=$(nix build .#iso-demo --no-link --print-out-paths)
python3 scripts/demo-3vm-iso.py "$(find "$out" -name '*.iso' | head -1)"
```

O runner sobe 3 QEMU (TCG/KVM conforme o host):

| Nó | VNC | SSH (hostfwd) | mesh |
|---|---|---|---|
| 1 | 127.0.0.1:5901 | 2201 | 192.168.50.1 |
| 2 | 127.0.0.1:5902 | 2202 | 192.168.50.2 |
| 3 | 127.0.0.1:5903 | 2203 | 192.168.50.3 |

- Rede de mesh: `-netdev socket,udp` + relay UDP em userspace (1 datagrama = 1
  frame); `listen/connect` do QEMU só aceita uma ligação e `mcast` não
  atravessa o WSL.
- NIC de controlo: user net do QEMU + `hostfwd` para SSH.

## O que o runner prova (última execução)

- `mesh_peers: 2` e `mesh_pinned: 2` nos **três** nós (TOFU `tofu: 1`).
- `crdt-put k1 v1` em 1 → `crdt-sync` 1→2 → `crdt-get` em 2 = `v1` →
  `crdt-sync` 2→3 → `crdt-get` em 3 = `v1`.
- `job-submit` 1→3: `ok=true`, `verified=true`, `text=[cpu-echo] oi demo 3vm`,
  `worker=<id do nó 3>`.
- `jarbas-cage` arrancado explicitamente por SSH (o firstboot pode ficar no
  transmute); `systemctl is-active` = active nos 3.
- **Captura**: `screendump` do virtio-gpu saiu **preto** com o cage active
  (VT/plane). O HUD foi observado via VNC e em execucao anterior; a mesh nao
  depende da captura.

## Limites

- QEMU ≠ metal (M4 `AWAITING_HW`); 3 VMs num só host, sem NAT/Wi-Fi.
- A imagem inclui o HUD/UI em curso de outra sessão (não versionado aqui).
- SSH com password e firewall aberto são **só** da demo; nunca promover esta
  imagem.
- O relógio do host/WSL e o TCG tornam o arranque lento; o runner espera pelo
  `jarbas-cage` antes do screenshot.
