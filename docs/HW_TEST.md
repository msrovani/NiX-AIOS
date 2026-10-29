# Teste em hardware real — `iso-hw` com logs persistentes

**Imagem:** `.#iso-hw` (não é a ISO de release; inclui o HUD/orb híbrido).
**Ficheiro gerado:** `C:\Users\msrov\Downloads\nixaios-hw-051.iso`
**SHA-256:** `1e90651a06899628bfa85b26fa55983f101ffb9d7143183f64f27ca100a3806c`
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
>
> Para Wi-Fi, criar `NIXAIOSLOG/wifi.conf` (texto simples):
>
> ```
> SSID=MinhaRede
> PSK=segredo
> # HIDDEN=1   (rede oculta)
> ```
>
> O `nixaios-wifi-seed` liga antes do firstboot; se falhar, o motivo fica em
> `wifi-diag.log`. Gravar o `wifi.conf` em UTF-8 **sem BOM** (Notepad/VS
> Code); comentários só em linhas próprias (`# ...`).
>
> O próprio questionário grava as respostas em `NIXAIOSLOG/firstboot.conf`
> (`DONE=1`, `LANG`, `KEYMAP`, `TZ`) e o Wi-Fi escolhido em `wifi.conf`.
> No **2º+ boot** reusa tudo e arranca em auto — há 10 s no tty1 para
> reconfigurar (Enter); `NOSD_FIRSTBOOT_ASK=1` no cmdline força o
> questionário. A PSK fica em texto no FAT: não usar um pendrive partilhado.

## 3. Arrancar e testar

Ligar **os dois** (ISO + logs), arrancar pelo pendrive da ISO. A `iso-hw` é
**interactiva**: o questionário Q1–Q5 aparece no tty1 (ecrã físico) antes do
transmute e do GUI. O Q1 lista SSIDs e pede a senha; se houver `wifi.conf`
no pendrive, o `nixaios-wifi-seed` liga antes e o Q1 mostra “Já ligado”.
O wrapper só dispara o firstboot no serial quando `NOSD_FIRSTBOOT_AUTO=1`
(ou `ASK=1`), para as perguntas não ficarem presas no getty serial.
`console=tty0` mantém o boot visível no ecrã.

No **2º+ boot**, se o pendrive tiver `firstboot.conf` com `DONE=1`, o
firstboot reusa as respostas e vai direito ao GUI (10 s de Enter no tty1
para reconfigurar). Apagar o `firstboot.conf` do pendrive também repõe o
questionário.

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

Fallback (sem GGUF no pendrive): no Q5, com rede, o transmute baixa via HF
(com GGUF seedado o ficheiro é cache hit). Sem rede e sem GGUF fica
`cpu-echo`.

**Limites**: o engine usa `/completion` (sem chat template); o Qwen3.5 em
modo *thinking* pode divagar — chat polido exige `/v1/chat/completions` ou
template explícito (residual). Visão (`mmproj-*.gguf`) ainda não está
ligada ao slot Vision. `NOSD_LLAMA_CTX=4096` está no drop-in do `nosd` (o
GGUF pede 262k, mas o KV é desnecessário aqui e atrasa/estoura o health de
30 s). Evidência de compatibilidade:
`docs/evidence/QWEN35_0.8B_0.6.0-alpha.12.md`.
