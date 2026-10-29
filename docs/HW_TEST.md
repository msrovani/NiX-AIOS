# Teste em hardware real — `iso-hw` com logs persistentes

**Imagem:** `.#iso-hw` (não é a ISO de release; inclui o HUD/orb híbrido).
**Ficheiro gerado:** `C:\Users\msrov\Downloads\nixaios-hw-051.iso`
**SHA-256:** `fe83b6e6639a5ade0d9bfeb68d286d293c687e0f87228927ba7f5ae1ef39e061`
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

## 3. Arrancar e testar

Ligar **os dois** (ISO + logs), arrancar pelo pendrive da ISO. O firstboot é
automático. No hardware, verificar:

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

O export arranca depois do `nosd`; o mount tenta 45 s (USB lento) e falha
“soft” (o boot continua, mas sem logs persistentes).

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

## Modelo de arranque: Qwen3.5-0.8B (primeiro boot)

A `iso-hw` inclui `llama.cpp` (a ISO de release n?o; por isso aparece
`engine=cpu-echo`) e traz um modelo de arranque configur?vel:

- **Repo/file**: `unsloth/Qwen3.5-0.8B-GGUF` / `Qwen3.5-0.8B-Q4_K_M.gguf`
  (**508 MB**, Apache-2.0). O repo `Qwen/Qwen3.5-0.8B` ? safetensors
  multimodal e **n?o** serve directamente ao llama.cpp.
- No primeiro boot, o `transmute` (com grant HITL) baixa o GGUF para
  `/var/lib/nosd/models/active-qwen3.5-0.8b-q4_k_m.gguf`, registra o slot
  Active e faz `hot_load`; `NOSD_BOOT_MODEL_ONLY=1` evita o coder extra.
- **Precisa de rede no primeiro boot** (cabo ou Wi-Fi interactivo com
  `NOSD_FIRSTBOOT_ASK=1`). Sem rede cai para `--no-download` e fica
  `cpu-echo`.
- Verificado nesta ?rvore: `llama.cpp 0.6.0` carrega a arquitectura
  `qwen35` e gera texto (Q4_K_M, CPU).
- **Limites**: o engine usa `/completion` (sem chat template); o Qwen3.5 em
  modo *thinking* pode divagar ? chat polido exige `/v1/chat/completions` ou
  template expl?cito (residual). Vis?o (`mmproj-*.gguf`) ainda n?o est?
  ligada ao slot Vision.
- `NOSD_LLAMA_CTX=4096` est? no drop-in do `nosd` (o GGUF pede 262k, mas o KV
  ? desnecess?rio aqui e atrasa/estoura o health de 30 s).
- Evid?ncia de compatibilidade: `docs/evidence/QWEN35_0.8B_0.6.0-alpha.12.md`.
