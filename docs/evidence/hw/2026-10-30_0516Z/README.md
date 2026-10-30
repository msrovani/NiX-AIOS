# HW image 0.6.0-alpha.16 — S1 0.8B (CPU) + S2/PRO 4B (CUDA)

Data: 2026-10-30 (build 04:50Z ISO / 05:11Z pack)
Base: `02c51b9` (inclui `14c1178` S1+S2 e `cb7ca01` routing V10)

## Artefactos

| Ficheiro | Bytes | sha256 |
|---|---|---|
| `nixaios-hw-cuda-051.iso` | 7 369 850 880 (6.87 GiB) | `c68c1a8a275dab584be490b1da858f8c9c608e27df845a28f2b50485a5a68f4c` |
| `nixaios-hw-cuda-051-logfat.img` | 11 665 408 000 (10.9 GiB) | `0274c982b296f013beabcd5e6c8f79b328237d8dc0e521ec4674b96c3bfdd613` |

Grafar **a `.img`** em modo DD (Rufus: "Write in DD image mode", ou `dd bs=16M`).
Gravar o `.iso` em modo ISO perde a partição `NIXAIOSLOG`.

## O que a imagem contém

- **S1 = Qwen3.5 0.8B Q4_K_M** (`Qwen3.5-0.8B-Q4_K_M.gguf`, 532 517 120 B) —
  `nosd` com `NOSD_LLAMA_NGL=0` e `NOSD_LLAMA_MODEL` pinado no 0.8B.
- **S2/PRO = Qwen3.5 4B Q4_K_M** (`Qwen3.5-4B-Q4_K_M.gguf`, 2 740 937 888 B) —
  unit `nosd-engine-s2` (127.0.0.1:8082, `-ngl 99`, `DeviceAllow` nvidia).
- Router V10 (`nos-core::routing::plan_backend`), persona em `/etc/nosd/SOUL.md`,
  `health.gpu {nvidia_smi, ngl, cuda_visible}`, `health.engine_s2`.

Prova de que os dois GGUF entram na ISO (derivação do `isoImage`):

```
Qwen3.5-4B-Q4_K_M.gguf.drv
Qwen3.5-0.8B-Q4_K_M.gguf.drv
unit-nosd-engine-s2.service.drv
etc-nosd-SOUL.md.drv
```

Prova de que o `nosd` na imagem tem o router: `strings nosd | grep -cE
's2_busy_interactive|prompt_too_long|s2_absent'` > 0.

## Verificação da `.img`

O ISO dentro do `.img` é bit-a-bit o ISO publicado, **excepto o setor 0** (MBR),
que o `sfdisk --append` reescreve para acrescentar a partição 3:

```
ISO_TAIL_SHA (bytes 512..)  = 80739ea5ce815d378dbeafabc6d3df6a30d00fd781b678add6eb9ce1cca386c9
IMG_TAIL_SHA (bytes 512..)  = 80739ea5ce815d378dbeafabc6d3df6a30d00fd781b678add6eb9ce1cca386c9
MBR_ISO = ebd98d93…   MBR_IMG = cc42baf9…   (esperado: tabela de partições)
```

Partição de log (`mdir -i img@@14395392*512`):

```
Volume in drive : is NIXAIOSLOG
README   txt       580
```

## Correções de build/pack nesta volta

1. `cp` de ~11 GB para `/mnt/c` (9p) morria com **ENOMEM** e deixava um `.img`
   truncado com o nome final → `dd bs=16M conv=fsync` para `<out>.part` + `mv`.
2. O `trap ... EXIT` do pack apagava o `.part` **no sucesso** (antes do `mv`) →
   só limpa em falha (`DONE_IMG`).
3. `.iso`/`.img` anteriores com atributo **ReadOnly** no drvfs fazem `dd`/`mv`
   falhar com `Permission denied` (`chmod` é no-op no 9p) → `attrib.exe -R`
   via `wslpath` antes de remover/renomear.

## Checklist de metal (o que ainda NÃO está observado)

```
nvidia-smi                                     # 3050 6 GB visível
nosctl health | grep -A3 gpu                   # nvidia_smi != null, ngl=0
nosctl health | grep -E 'engine|engine_s2'     # engine_s2 != null no PRO
systemctl status nosd-engine-s2                # active (4B em 8082)
journalctl -u nosd-engine-s2 -b | grep -iE 'offload|CUDA|using CPU'
nosctl route "implemente um scheduler"         # plan.backend/plan.reason
```

Esperado: S1 responde no chat curto (0.8B, CPU, `lane_s1`); pergunta profunda
vai a S2 (`lane_s2`) e o journal do `nosd-engine-s2` diz `offloaded 33/33
layers`. Se disser `using CPU` com `-ngl 99`, é driver/allowlist, não o modelo.

## Residual honesto

- Sem OBSERVED em metal para: VRAM real do 4B, latência S2, routing ponta-a-ponta.
- `nosd-engine-s2` **sem `--api-key`** (loopback only).
- Admissão de VRAM é feita uma vez, no arranque do serviço (sem cache/TTL).
- QEMU não prova CUDA (virtio-gpu sem passthrough de NVIDIA).
