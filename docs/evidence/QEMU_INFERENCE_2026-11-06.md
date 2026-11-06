# Prova QEMU + argv do motor sem `-ngl` (F1/F2 em imagem, 2026-11-06)

**Data:** 2026-11-06 · **Plano:** [`docs/PLAN_INFERENCE_VRAM_VOZ_FABRICA.md`](../PLAN_INFERENCE_VRAM_VOZ_FABRICA.md)
**Estado:** QEMU **OBSERVED** (lab, `≠ metal` ADR-0113). Metal e lane CUDA **NOT-TESTED**.

Pergunta do dia: depois de F1–F7 no `main`, o `nosd` **deixa de escrever `-ngl`**
no motor e carrega **uma só** cópia do modelo? Aqui ficam as duas provas que o
host consegue dar — uma na imagem (QEMU) e outra no processo real com o 4B.

## ISO de prova

`nix build .#iso-hw` a partir de `main` **depois** de F1–F7 (árvore limpa, sem
`--impure`: o flake viu o `HEAD` com todos os commits).

| Atributo | Valor |
|---|---|
| ISO | `nixos-minimal-26.11.20261020.02c087f-x86_64-linux.iso` |
| bytes | `7 050 903 552` |
| sha256 | `dbe9d0e5bf952c8832efa8daba63c37ef63fc861bc92bdbee670b8c6c894caa3` |
| build | `ISOBUILD_RC=0` |

## 1. Gate `--stage MQEMU`

```
ISO=<iso> MEM=4G SMP=4 DISPLAY_MODE=serial LOG=$HOME/qemu_boot.log \
  bash scripts/run-iso-qemu-wsl.sh
NIXAIOS_QEMU_LOG=$HOME/qemu_boot.log bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```

Resultado literal:

```
stage=MQEMU ring=Ring3_Linux_qemu layer=DELEGATE claim_status=OBSERVED result=PASS detail=qemu_serial_kernel_boot_markers log=/home/msrov/qemu_boot.log
stage=MQEMU ring=Ring3_Linux_qemu layer=ADAPT claim_status=UNKNOWN result=SKIP detail=nosd start status unclear in qemu log
gate_summary fails=0 stage=MQEMU strict=0 idea=#707 playbook=docs/BOOT_TO_JARBAS.md
```

`fails=0`, mas **o `SKIP` não é "não aplicável"**: é uma **cegueira do gate**.
A ISO faz auto-login em `ttyS0` (`nixos login: nixos (início de sessão
automático)`) e o shell de bootstrap toma a consola antes de o systemd imprimir
`Started Neural-OS Daemon`; o gate só lê o log de série, logo nunca vê a linha.
O daemon **estava** a correr (abaixo). Corrigir isto é mudar o gate (aceitar
`journalctl -b -u nosd` como fonte), não maquilhar o log — log maquilhado é o
`grep NixOS` do menu que a lição do próprio script já proíbe.

## 2. O que o guest respondeu (socket de série)

`run-iso-qemu-wsl.sh` liga a série a `stdio`, que aqui é `/dev/null` (nohup) —
não dá para perguntar nada. Nesta prova o QEMU foi arrancado com
`-serial unix:$HOME/ser.sock,server,nowait -display none` e um cliente Python
enviou os comandos. Perguntas e respostas, texto literal:

```
nixos@nixaios:~]$ systemctl is-enabled nosd          -> enabled
nixos@nixaios:~]$ systemctl is-active nosd jarbas-cage nosd-early-score
                                                     -> active / inactive / active
nixos@nixaios:~]$ systemctl status nosd --no-pager -n 4
   Active: active (running) since Thu 2026-11-05 22:53:07 -03; 7min ago
   Memory: 2.8G (max: 16G, available: 793.2M, peak: 2.8G)
```

Marcadores **novos** (não existem no binário anterior — é isto que prova que a
imagem tem F1/F2):

```
INFO nosd: capacity ram_avail_mb=3003 vendor=cpu cards=["card0"] render=[] igpu=None dgpu=None vram_free_mb=0 vram_total_mb=0 posture=no_gpu_hint suggest_ngl=0 (hint only — --fit decides; set NOSD_LLAMA_NGL in the unit to override)
INFO nosd: hub slot=active conflict keep=/var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf ignored=/var/lib/nosd/models/qwen3.5-0.8b-q4km.gguf
INFO nosd: llama-server spawned pid=1221 base=http://127.0.0.1:33753 model=/var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf ngl=0 thermal_cap=None api_key=set
INFO nosd: engine=llama-server-http
```

`conflict keep=… ignored=…` é a F2 (`register_path` com `candidates`/`conflict`)
a resolver dois nomes para o mesmo slot — antes registava os dois em silêncio.

### argv real dentro do guest

```
1221 llama-server /nix/store/wll87va6cjhqjk8gmvzcmipvfwj234pd-llama-cpp-0.6.0/bin/llama-server \
  --host 127.0.0.1 --port 33753 -m /var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf \
  --api-key <redacted> --no-webui --fit on --fit-ctx 2048 --reasoning off
```

**Sem `-ngl`.** Um único processo (`pgrep -af llama` → 1 linha, pid 1221), com o
S1 0.8B. O 4B é **declarado** (`hub register generator_pro
…/Qwen3.5-4B-Q6_K.gguf`) mas **não é spawnado**: a `iso-hw` não traz esse GGUF e
o guest não tem GPU — nenhuma segunda cópia existe por construção.

## 3. O 4B, num processo real (host, WSL)

O guest de 4 GiB **não pode** provar o 4B: o Q6_K tem 5,4 GB e o host só tem
7 GiB. A prova do 4B veio do binário novo a correr neste host, com o GGUF real
(`0cpj…-Qwen3.5-4B-Q6_K.gguf`, `NOSD_LLAMA_MODEL` apontado para ele):

```
INFO nosd: capacity ram_avail_mb=5848 vendor=cpu … posture=drm_present_vram_unknown suggest_ngl=0 (hint only — --fit decides; …)
INFO nosd: llama-server spawned pid=612 base=http://127.0.0.1:44153 model=/home/msrov/nosd-lab/data/models/4b.gguf ngl=0 thermal_cap=None api_key=set
```

argv lido de `/proc/612/cmdline`:

```
llama-server --host 127.0.0.1 --port 44153 -m …/4b.gguf --api-key <redacted>
  --no-webui --fit on --fit-ctx 2048 --reasoning off
ngl_args=0
llama_server_procs=1
```

Antes de F1 este mesmo caminho escrevia `NOSD_LLAMA_NGL` no env do daemon e o
sitio do `-ngl` era o argv (`-ngl 0/16/99` conforme heurística/térmica). Agora
o `-ngl` não aparece e `--fit on` fica sozinho a decidir as camadas.

## O que NÃO fica provado aqui

- **Metal** (Core 7 / i5): nada. `QEMU ≠ metal` (ADR-0113).
- **Lane CUDA/S2 com o 4B na GPU**: o QEMU não tem NVIDIA. O "uma só cópia"
  está provado para o processo que existe; a corrida S1(CPU 0.8B) + S2(CUDA 4B)
  em simultâneo continua `AWAITING_HW`.
- **`jarbas-cage`**: `inactive` no guest (sem GPU/VT); o HUD não foi visto.
- **Voz**: nenhum WAV tocado; a ISO traz piper/pt_BR e `ggml-tiny.bin`, não
  foram exercitados.
- **Gate**: o `SKIP` do `nosd` é a cegueira do log de série descrita acima;
  fechar isso exige mudar o gate (não feito neste passo).

## Repro

```bash
# 1. imagem (árvore limpa = HEAD)
nix build .#iso-hw --out-link ~/nixaios-iso-hw

# 2. guest com série em socket (TCG se o utilizador não estiver no grupo kvm)
qemu-system-x86_64 -cpu qemu64 -m 4G -smp 4 -cdrom <iso> -boot d \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0 \
  -display none -serial unix:$HOME/ser.sock,server,nowait -monitor none

# 3. interrogar o guest pelo socket (python: connect A F_UNIX, send "systemctl is-active nosd\n")

# 4. gate
NIXAIOS_QEMU_LOG=$HOME/qemu_boot.log bash scripts/gate-boot-to-jarbas.sh --stage MQEMU
```
