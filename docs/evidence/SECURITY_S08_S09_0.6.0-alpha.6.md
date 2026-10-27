# Evidence — S-08/S-09 (0.6.0-alpha.6)

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux/qemu` + `Ring3_Linux` VM — **OBSERVED** (não metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/63fp0h7sdpdfp2p8r9di1v73n8pjqcis-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**SHA-256:** `592a95d053b31ffaf2dd3cab02fa18937ab9217fc51dc23299e0fd366a6ace1c`

## S-09 — socket `0660` no ISO real

Boot da ISO alpha.6 em QEMU (`serial`, `nosd` a correr):

```
$ echo ACTIVE_NOSD=$(systemctl is-active nosd)
ACTIVE_NOSD=active
$ sudo stat -c "%a %U:%G" /run/nosd/nosd.sock
660 nosd:nosd
```

Gate:

```
stage=MQEMU ... result=PASS detail=qemu_serial_boot_markers
stage=MQEMU ... result=PASS detail=nosd_start_ok_in_qemu_log
gate_summary fails=0
```

## VM test (`checks.x86_64-linux.nixaios-vm`)

`nix build .#checks.x86_64-linux.nixaios-vm` → PASS (141 s de test script).
Inclui a asserção nova:

```
$ test "$(stat -c %a /run/nosd/nosd.sock)" = 660
```

(buildRustPackage corre também `cargo test` em Linux, incluindo
`ipc_auth::unix_socket_is_0660`, que é `#[cfg(unix)]`.)

## S-08 — suite

```
cargo test --workspace
  nos-ipc     : 3 passed   (roundtrip, oversized frame, DENY_UNKNOWN)
  nosd (lib)  : 21 passed
  ipc_auth    : 10-11 passed (unix_socket_is_0660, unknown_method,
                             admin_method_requires_token, grants, rotação, ring)
cargo clippy --workspace --all-targets --all-features -- -D warnings  -> ok
cargo fmt --all --check                                             -> ok
```

## Honestidade

- O modo `0660` é defesa em profundidade; a autorização real continua a ser
  `SO_PEERCRED` + caps no servidor.
- Isto não é uma auditoria ofensiva nem prova contra um atacante com root.
- `NOSD_NIX_RING` continua desarmado (R-03/R-04 pendentes); S-04 legacy
  (`hitl_confirmed` só para LocalAdmin) continua a existir.
