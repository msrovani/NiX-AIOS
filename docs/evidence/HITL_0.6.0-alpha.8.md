# Evidence — S-04 fechado (HITL só por grant) 0.6.0-alpha.8

**Data:** 2026-10-27  
**Ring:** `Ring3_Linux/qemu` — **OBSERVED** (não metal)  
**Volid:** `NIXAIOS051`  
**ISO:** `/nix/store/nff2chhnbgp1d6h6phkcd63yc3f8x3lk-nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso/iso/nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso`  
**SHA-256:** `5cae5d748decef01b0decf347dc98d56dccbd9e35901c63ed2ce478d2b911a9d`  
**Screenshot:** [`hitl-0.6.0-alpha.8.png`](hitl-0.6.0-alpha.8.png)

## 1. O booleano do cliente deixou de autorizar

`grep hitl_confirmed` no servidor: só existe em comentários/estruturas internas
(`blob_deliberate`, `actuators`) — **nenhum** handler lê o parâmetro do wire.

```
$ grep -rn 'hitl_confirmed' packages/nosd/crates/nosd/src
rpc/mod.rs:  (comentário: "ignorado")
```

Testes (`cargo test -p nosd --test ipc_auth`, 11 PASS):

| Teste | Prova |
|---|---|
| `client_hitl_boolean_is_never_consent` | token admin + `hitl_confirmed:true` → `HITL required` |
| `client_hitl_boolean_is_never_consent` (2ª parte) | grant de `wifi.connect` em `transmute.run` → negado (kind-bound) |
| `nix_ring_is_disarmed_by_default` | com grant válido → `NIX_RING_DISABLED` (não HITL) |
| `hitl_grant_is_single_use_and_kind_bound` | replay negado |
| `blob_fetch_requires_single_use_grant` | fetch sem grant negado; com grant passa o HITL |

## 2. Cadeia real na ISO (boot → grant → transmute → HUD)

Sem comandos no guest até ao fim (`firstbootAuto = true`):

```
HITL grant obtido (kind=transmute, single-use)
— Transmute (HITL grant + download MoE/Whisper/Piper + hot_load) —
transmute RPC OK
  note=Downloads OK; hotswap disparado (log /var/lib/nosd/hotswap.log); models=/var/lib/nosd/models.
=== nos-firstboot DONE (GUI up) ===
firstboot.done + jarbas-cage active
```

## 3. Casos negativos na ISO (root, LocalAdmin)

```
$ sudo nosctl ... transmute --no-download --no-hotswap
  "ok": false,
  "error": "HITL required: confirm_id from ui.action (kind=transmute)"

$ sudo nosctl ... transmute --no-download --no-hotswap --confirm-id deadbeef
  "ok": false,
  "error": "HITL required: confirm_id from ui.action (kind=transmute)"
```

Ou seja: **nem root** passa sem um grant válido emitido pelo servidor.

## 4. Gate

```
stage=MQEMU ... layer=DELEGATE OBSERVED PASS detail=qemu_serial_boot_markers
stage=MQEMU ... layer=ADAPT    OBSERVED PASS detail=nosd_start_ok_in_qemu_log
gate_summary fails=0
```

## 5. Bug apanhado pelo VM test: `nosctl` + `grep -q`

O test `chat | grep -q` falhou com `panic: failed printing to stdout:
Broken pipe`. O `println!` do `nosctl` entrava em pânico quando o consumidor
fechava o pipe (`grep -q`, `head`, `less`). Corrigido com `print_json()`:
`BrokenPipe` é sucesso silencioso (commit do alpha.8).

## Honestidade

- O grant **não é prova de humano**: um LocalAdmin pode chamar
  `nosctl consent <kind>`. O que mudou é que a autorização passou a ser
  server-authored, single-use e kind-bound, em vez de um booleano no payload.
- Contra root local, nada disto protege (root edita a unit/binário).
- Em modo auto (ISO/lab) o consentimento é a política `firstbootAuto` da
  imagem — documentado, não é chamado de HITL.
- Ring Nix continua desarmado (R-03/R-04); este fecho remove um dos critérios,
  não arma nada.
