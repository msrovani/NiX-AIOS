# Evidence — M6 identidade de nó (HB assinado)

**Data:** 2026-10-27  
**Estado:** implementado e provado em VM (não é release tag; entra na release
de frota alpha.9 quando M7–M9 fecharem).  
**Âmbito:** identidade Ed25519, HB assinado, pinagem TOFU, anti-replay,
allowlist, recusa de HB sem assinatura.

## O que mudou

| Peça | Antes | Agora |
|---|---|---|
| id do nó | `--node-id` escolhido | **derivado** da chave (`fnv1a64(pk)[..32]`) |
| HB | `HB\0 id PK\0 CAP\0 cpu mem net` | + `seq:u64` + `KEY\0 pk32` + `SIG\0 sig64` |
| verificação | nenhuma | Ed25519 sobre todo o HB até `SIG\0` |
| replay | nenhum | `seq` monotónico por peer (janela = último visto) |
| allowlist | nenhuma | `NOSD_MESH_ALLOWLIST=1,2,3` (vazia = TOFU) |
| modo | sempre aberto | `NOSD_MESH_REQUIRE_SIG=1` recusa HB antigo |

Ficheiros: `crates/nos-mesh/src/ident.rs` (novo), `lib.rs` (wire + gate),
`crates/nosd/src/main.rs` (arranque da identidade), `rpc/health.rs`
(`mesh_pinned`, `mesh_node_id`).

Fail-closed: com `NOSD_MESH_REQUIRE_SIG=1` e chave ilegível, o `nosd` **não
arranca** em vez de aceitar HB aberto.

## Testes (host)

```
cargo test -p nos-mesh        -> 35 passed
  ident::id_is_derived_from_key
  ident::sign_verify_roundtrip_and_tamper
  ident::pinning_rejects_key_change_and_replay
  ident::allowlist_filters_ids
  tests::signed_hb_is_verified_and_pinned
  tests::tampered_hb_is_rejected
  tests::unsigned_hb_rejected_when_signature_required
  tests::allowlist_blocks_unknown_signed_peer
cargo clippy --workspace ... -D warnings -> ok
```

## Teste de frota (`checks.x86_64-linux.mesh-2vm-signed`)

2 VMs, VLAN 1, `NOSD_MESH_REQUIRE_SIG=1` nos dois:

| Asserção | Resultado |
|---|---|
| `mesh_peers: 1` nos dois | PASS |
| `mesh_pinned: 1` nos dois (chave fixada) | PASS |
| HB antigo/sem assinatura injetado (unicast, id 99) **não** cria peer | PASS |
| daemon regista `sem assinatura` | PASS |

## Honestidade / residual

- **Sem confidencialidade**: o HB é assinado mas vai em claro (UDP). Noise
  (M6.1) é o passo seguinte; para trabalho (jobs) o transporte será TCP+Noise.
- **Allowlist por id derivado**; a chave em si é TOFU (primeira vista). Num
  deploy real, fixar a chave no provisionamento (nix `environment.etc` ou TPM).
- **Não há revogação** ainda (`REVOKE` está no plano, M6.2).
- O HB continua a ser *descoberta*; não autoriza trabalho — isso é M7.
- Sem chave configurada, o comportamento é o de alpha.8 (compat lab), para o
  ISO atual não mudar.
