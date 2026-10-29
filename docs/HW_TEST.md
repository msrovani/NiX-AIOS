# Teste em hardware real — `iso-hw` com logs persistentes

**Imagem:** `.#iso-hw` (não é a ISO de release; inclui o HUD/orb híbrido).
**Ficheiro gerado:** `C:\Users\msrov\Downloads\nixaios-hw-051.iso`
**SHA-256:** `734bc23f75b7f5c793801554f72a8b555bec3e4c1927773977f1087ba9cfa6d9`
**Volid:** `NIXAIOS051` · `makeUsbBootable = true`

## 1. Gravar no pendrive

**Windows:** Rufus → *DD mode* (não ISO mode) → selecionar o `.iso` → escrever.
**Linux/WSL:**

```bash
sudo dd if=nixaios-hw-051.iso of=/dev/sdX bs=4M status=progress conv=fsync
sync
```

`/dev/sdX` é o **disco inteiro** do pendrive. Isto apaga-o.

## 2. Partição de logs (FAT32, legível no Windows)

Use um **segundo** pendrive (recomendado) ou uma partição livre, formatada
FAT32 com o label exacto:

**Windows (segundo pendrive):** formatar em FAT32 e pôr o nome `NIXAIOSLOG`
(diskpart: `format fs=fat32 label=NIXAIOSLOG quick`).
**Linux:**

```bash
sudo mkfs.vfat -n NIXAIOSLOG /dev/sdY1
```

> A ISO procura `LABEL=NIXAIOSLOG`; se não encontrar, tenta o maior VFAT >100 MB
> que não seja EFI (fallback antigo). Num PC com outros USBs FAT, use o label
> correcto e não deixe outro pendrive FAT ligado.
>
> Para inferência offline, criar `NIXAIOSLOG/models/` e lá colocar o GGUF
> (ver secção “Modelo de arranque” abaixo).

## 3. Arrancar e testar

Ligar **os dois** (ISO + logs), arrancar pelo pendrive da ISO. No metal o
firstboot é **interactivo** (Q1-Q5): a `iso-hw` força `firstbootAuto=false`
para o Q1 listar SSIDs e pedir a senha Wi-Fi. (O modo auto esperava 8 s, não
perguntava rede e caía em `--no-download` — foi o sintoma de 28/10.)

No hardware, verificar:

- cage/HUD no iGPU real (não `pixman`/`llvmpipe` de QEMU) e VT/teclado físicos;
- firstboot/transmute/hotswap;
- Wi-Fi/áudio/câmara, se aplicável;
- `EngineAbsent`/sem GGUF continua a ser honestidade (não é falha).

## 4. O que fica na partição de logs

| Ficheiro | Conteúdo |
|---|---|
| `journal-boot.log` | snapshot do journal (nosd, jarbas-cage/hub, hotswap, log-mount/export) |
| `journal-follow.log` | `journalctl -f` contínuo dos mesmos units |
| `firstboot.log`, `hotswap.log`, `jarbas-cage.log` | cópia a cada 20 s de `/tmp/nos-firstboot.log` e `/var/lib/nosd/*.log` |
| `dmesg-boot.log` | dmesg do arranque |
| `hw-info.log` | `uname`, `/proc/cmdline`, `lspci -nn`, `lsusb` |
| `wifi-diag.log` | `rfkill`, `nmcli device`, scan SSID e firmware no arranque |
| `model-seed.log` | cópia dos GGUF do pendrive para `/var/lib/nosd/models` |

O export arranca depois do `nosd`; o mount tenta 45 s (USB lento). Se a FAT
não existir, o serviço **falha alto no tty1** e re-tenta a cada 10 s
(`Restart=on-failure`) — em vez de sair em silêncio como antes. Sem FAT os
logs ficam só em RAM e perdem-se no poweroff.

## 5. Recolher para análise

Depois do teste, montar a partição no Windows e copiar **tudo** para
`docs/evidence/hw/<data>/` no repo (ou zip). Não commitar segredos: o
`journal-follow.log` pode conter hostnames/IPs.

```bash
mkdir -p docs/evidence/hw/$(date -u +%Y%m%d)
cp -a /media/$USER/NIXAIOSLOG/. docs/evidence/hw/$(date -u +%Y%m%d)/
```

## Limites

- FAT32 tem limite de 4 GiB por ficheiro; `journal-follow.log` cresce. Para
  sessões longas, parar/rodar o ficheiro ou usar outra partição.
- A imagem é de diagnóstico: não abre SSH nem firewall (igual à release).
- QEMU ≠ metal; este é o único caminho para M4. `AWAITING_HW` até haver
  evidência com `docs/evidence/hw/`.
- O HUD/orb híbrido ainda não foi observado em metal (só bancada/QEMU).

## Modelo de arranque: Qwen3.5-0.8B (offline primeiro)

A `iso-hw` inclui `llama.cpp` (a ISO de release não; por isso aparece
`engine=cpu-echo`). No metal o caminho normal é **offline**:

1. Copiar o GGUF para o pendrive de logs em `NIXAIOSLOG/models/` (ou na
   raiz). Nome esperado: `active-qwen3.5-0.8b-q4_k_m.gguf` (qualquer
   `*.gguf` serve; o slot deriva do nome).
2. No boot, `nixaios-model-seed.service` copia para `/var/lib/nosd/models`
   **antes** do `nosd`/firstboot, com verificação de espaço e tamanho.
3. O `transmute` regista o GGUF e faz `hot_load` — sem rede.

Origem: `unsloth/Qwen3.5-0.8B-GGUF` / `Qwen3.5-0.8B-Q4_K_M.gguf`
(**508 MB**, Apache-2.0). O repo `Qwen/Qwen3.5-0.8B` é safetensors
multimodal e **não** serve directamente ao llama.cpp.

Fallback (sem GGUF no pendrive): o firstboot interactivo pode baixar via HF
(Q5 + grant HITL). Sem rede cai para `--no-download` e fica `cpu-echo`.

**Limites**: o engine usa `/completion` (sem chat template); o Qwen3.5 em
modo *thinking* pode divagar — chat polido exige `/v1/chat/completions` ou
template explícito (residual). Visão (`mmproj-*.gguf`) ainda não está
ligada ao slot Vision. `NOSD_LLAMA_CTX=4096` está no drop-in do `nosd` (o
GGUF pede 262k, mas o KV é desnecessário aqui e atrasa/estoura o health de
30 s). Evidência de compatibilidade:
`docs/evidence/QWEN35_0.8B_0.6.0-alpha.12.md`.
