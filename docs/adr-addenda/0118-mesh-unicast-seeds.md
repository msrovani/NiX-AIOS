# ADR-0118 addendum — Mesh: sementes unicast (`--mesh-peer`)

**Estado:** implementado e provado em host (2026-11-02, `0.7.0-alpha.5`
in-tree; ANVIL: `udp.rs`/`main.rs`/Nix/docs; FORGE: visibilidade
`health.mesh_seeds`). **Commit pendente** de autorização HUMAN (`from=HUMAN`
no fórum).  
**Substitui:** nada. **Relaciona:** M5/M5.1 (frota), M6 (identidade/TOFU),
`docs/SKYNET_DISTRIBUTED_PLAN.md` (“HB unicast para a allowlist; broadcast só
em lab”), lição 16 do `AGENTS.md`.

## Contexto

O `nosd` tem **um** destino de heartbeat: `--mesh-broadcast`
(`NOSD_MESH_BROADCAST`, default `255.255.255.255:19000`), e o próprio help do
argumento diz “broadcast/unicast”. `MeshUdp` envia HB, extras de tick, FRAG e
FRACK sempre para esse único `SocketAddr`.

No metal (0.7.0-alpha.4) a frota ficou com `peers=0`. Causas plausíveis, todas
já conhecidas:

- Wi‑Fi com **AP isolation**: broadcast não atravessa o AP.
- `255.255.255.255` sem rota → `ENETUNREACH` (lição 16); a resolução para o
  broadcast da sub-rede só cobre o caso “mesma LAN, sem isolation”.
- NAT / segmentos diferentes: broadcast nunca chega.

Ou seja: um único destino broadcast não é uma topologia de frota; é um caso
particular de lab.

## Decisão

1. Aceitar **N sementes unicast** para além do broadcast:
   - CLI: `--mesh-peer <ip:port>` repetível (e vírgulas no mesmo argumento);
   - env: `NOSD_MESH_PEERS` (lista separada por vírgulas);
   - NixOS: `services.nosd.meshPeers = [ "192.168.1.50:19000" ... ]` →
     `--mesh-peer` no `ExecStart` + `NOSD_MESH_PEERS` (health/config).
2. O conjunto de transmissão passa a ser
   `destinos = {broadcast} ∪ {sementes}`, **deduplicado**, e é usado por HB,
   extras de tick, FRAG e FRACK.
3. **Fail-closed no arranque**: semente inválida (`ip:port` que não faz parse) é
   erro de `bind()`, não é ignorada em silêncio.
4. Erros por destino são independentes: uma semente morta não impede o envio ao
   broadcast nem às outras sementes. O streak `ENETUNREACH` só conta quando
   **todos** os destinos falham.
5. `health`/`config.show` expõem `mesh_peers` (endereços, nunca chaves). Sem
   sementes o comportamento é byte-idêntico ao actual.
6. Sem mudança de wire format, de assinatura HB, de TOFU/allowlist ou de caps.

## Fora de escopo

- Descoberta automática (mDNS/`getifaddrs` scan) e NAT traversal — outra ADR.
- Gossip de sementes entre peers.
- TTL de peers / Merkle/bloom (M5.1 residual).
- Religar ring Nix, BitNet nativo, Ring0, X11.

## Critérios de aceite

- Teste unitário de `destinations()`: dedupe (semente igual ao broadcast),
  IPv4/IPv6 literais, rejeição de `ip` sem porto e de lixo.
- Teste de loopback A/B: duas `MeshUdp` em `127.0.0.1:0` com sementes cruzadas;
  `tick_tx()` em A → B vê o peer (sem broadcast; broadcast aponta para uma
  porta fechada para provar que a semente é o caminho).
- `cargo fmt --check`, `clippy -D warnings` e `cargo test --workspace` verdes.
- Log de arranque mostra `targets=N (broadcast=…, peers=…)`; `health` mostra a
  lista configurada.
- Evidência em `docs/evidence/` só depois de existir execução real (QEMU/lab ou
  2 instâncias host), com `Tested:`/`Not-tested:`.

## Consequências

- A frota passa a ter um caminho determinístico fora do broadcast (Wi‑Fi e
  segmentos roteados), sem depender de multicast.
- Operador tem de manter a lista de seeds (config declarativa no NixOS);
  seeds mortas custam um `send_to` falhado por tick (barato, sem log spam).
- O default (lista vazia) mantém o lab e a ISO de release inalterados.
