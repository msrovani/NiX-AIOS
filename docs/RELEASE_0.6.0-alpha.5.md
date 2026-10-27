# NixAIOS 0.6.0-alpha.5 — Boot→firstboot→JARBAS (headless/lab)

**Volid ISO:** `NIXAIOS051`  
**Workspace:** `packages/nosd` 0.6.0-alpha.5  
**Data:** 2026-10-27  
**Tag:** `v0.6.0-alpha.5`  
**Base:** 0.6.0-alpha.4

## Aceite

A ISO passa de “arranca e espera por um humano” para **boot → firstboot
automático → hotswap → JARBAS Slint** sem tocar no teclado, que é o que o
gate M3/QEMU precisa. Num install real o questionário continua interactivo.

## Correções

1. **firstboot nunca corria em `ttyS0`:** os padrões eram
   `/dev/tty[0-9]*`; a ISO arranca com `console=ttyS0` e o auto-login cai numa
   sessão serial. Adicionado `ttyS*`/`hvc*` nos três gatilhos
   (`nixos-aios-shell`, `loginShellInit`, `.bash_profile`).
2. **Modo automático (`services.nixaiosTransmute.firstbootAuto`)**: defaults
   para Q1–Q5, espera de rede 8 s, `transmute --no-download` (sem descarregar
   GBs em CI). `NOSD_FIRSTBOOT_ASK=1` força interactivo;
   `NOSD_FIRSTBOOT_DOWNLOAD=1` permite download em auto. A ISO usa `true`.
3. **`nosd-hotswap-aios.service` morria com 127:** o script é
   `#!/usr/bin/env bash` e o unit não tinha `path` — nem `bash`, nem `tee`,
   nem `systemctl` resolviam (`env: 'bash': Arquivo ou diretório
   inexistente`). `path` explícito no unit (e no `nosd-boot-persist-check`).
4. **hotswap reiniciava o `nosd` que servia o pedido:** o `transmute` chega
   por RPC ao próprio nosd; o `systemctl restart nosd` matava a resposta
   (`nosctl exit=1`). Agora só reinicia se o health estiver offline.
5. **`transmute` reportava falha de hotswap:** `nosd` corre como user `nosd`;
   `systemctl start` exige root/polkit e devolvia 1. O mecanismo é o ficheiro
   `hotswap-aios.request` + `nosd-hotswap-aios.path` (PathExists); o
   `systemctl` passou a best-effort (warn, não erro).
6. **`nix build` a seguir a `cargo test` no host falhava** com
   `cp: target/...: Read-only file system`: o `packages/nosd/target/` (GBs)
   era copiado read-only para a store. `src` passou a
   `builtins.path { filter = ... }` a excluir `target/.direnv/result`.
7. **Primeiro frame preto no arranque automático:** o wlroots/seatd só faz
   page-flip na VT activa; quando o `cage` arrancava com o firstboot em VT1 o
   ecrã ficava preto até um `systemctl restart jarbas-cage`. O
   `ExecStartPre` (root) faz `chvt` para a VT do kiosk antes do `ExecStart`.

## Evidência

[`evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md`](evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md)

```
modo=auto net_wait=8s
transmute RPC OK
note=Downloads OK; hotswap disparado (log /var/lib/nosd/hotswap.log)
=== nosd-hotswap-aios DONE (jarbas-cage active, VT7) ===
=== nos-firstboot DONE (GUI up) ===
firstboot.done + jarbas-cage active
```

ISO: SHA-256 `13d2a7bc096f1fecc70749bef06db07a12adf81b8f2c637c4116ba857465df77`;
gate `MQEMU` PASS (`fails=0`); screenshot do scanout no fim da cadeia,
sem `systemctl restart`.

## Honestidade

- Continua **QEMU** (`Ring3_Linux/qemu`, OBSERVED), não metal.
- O HUD em QEMU usa o fallback `winit-software` + `WLR_RENDERER=pixman`;
  GL/femtovg em iGPU real continua por verificar.
- Auto-firstboot é política de **ISO/lab**; num install real o questionário
  é o caminho (default `firstbootAuto = false`).

## Residual

- M4 metal, Wi-Fi/DRM/áudio/webcam reais.
- S-08/S-09; R-03/R-04 (ring desarmado).
- Sessão serial e VT partilham teclado: falta um “modo demo” com teclado
  virtual no HUD (slint), hoje só por `nosctl`.
