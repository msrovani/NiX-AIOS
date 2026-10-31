# DeviceAllow NVIDIA (metal CUDA) — fix estático

Data: 2026-10-30 · Host: WSL (nix 2.35.3) · Estado: host/eval OBSERVED, metal CUDA AWAITING_HW.

## Bug

`modules/nosd-engine.nix` e `images/hw-cuda.nix` listavam
`char-nvidia-frontend rw` e `char-nvidia-ctl rw`. Aliases de `DeviceAllow`
resolvem por `/proc/devices` (Linux, enforcement systemd/cgroup):

- `char-nvidia-frontend` só existe com o módulo vGPU/Xen — não existe em
  metal (RTX 3050);
- `char-nvidia-ctl` não é alias — `/dev/nvidiactl` é o minor 255 do char
  `nvidia` (major 195);
- faltava `char-nvidia rw` (cobre `/dev/nvidiaN` + `/dev/nvidiactl`).

Sob política `closed` implícita pelo `DeviceAllow` (`char-drm` no unit base,
/neural-os.nix/), o systemd **ignora os aliases desconhecidos em silêncio**
e a CUDA morre a abrir `/dev/nvidia0` (Permission denied → ENODEV).
O sintoma registado em `docs/evidence/hw/2026-10-29_1413Z` era a motivação
do fix, mas a lista antiga veio de boot QEMU — nunca provada em metal.

## Fix

- `modules/nosd-engine.nix` (2 units): `DeviceAllow = [ "char-nvidia rw"
  "char-nvidia-uvm rw" "char-nvidia-caps rw" ]` + comentário `/proc/devices`.
- `images/hw-cuda.nix` (units `nosd` e `nosd-engine-s2`): idem.
- `modules/jarbas-cage.nix`: comentário no `pickDrm` — AMD (0x1002) entra no
  ramo "não-NVIDIA" por omissão (APU-only apanha o card certo; nenhuma UI/
  inferência AMD validada, inferência hoje = CUDA/NVIDIA).
Sob política `closed` implícita pelo `DeviceAllow` (`char-drm` no unit base,
modules/neural-os.nix), o systemd **ignora os aliases desconhecidos em</think><tool_call>edit<arg_key>newString</arg_key><arg_value>- `flake.nix`: novo check estático `checks.x86_64-linux.hw-cuda-deviceallow`
  (eval da config hw-cuda + afirma `char-nvidia rw` presente e `frontend|ctl`
  ausente nos dois units).

## Prova (host Linux)

```
$ nix-instantiate --parse {flake.nix,modules/nosd-engine.nix,images/hw-cuda.nix,modules/jarbas-cage.nix}
  -> parse OK (4/4)
$ nix build .#checks.x86_64-linux.hw-cuda-deviceallow  -> RC=0
$ nix eval --json .#nixosConfigurations.nixaios-iso-hw-cuda.config.systemd.services.nosd.serviceConfig.DeviceAllow
["char-drm rw","char-nvidia rw","char-nvidia-uvm rw","char-nvidia-caps rw"]
$ nix eval --json ...services."nosd-engine-s2".serviceConfig.DeviceAllow
["char-nvidia rw","char-nvidia-uvm rw","char-nvidia-caps rw"]
```

## Not-tested

- CUDA real em metal (VM test não prova: virtio-gpu sem passthrough; ver
  `docs/evidence/hw/2026-10-30_0516Z` checklist: `journalctl -u
  nosd-engine-s2 -b | grep -iE 'offload|CUDA|using CPU'`);
- `nvidia-smi` sob o unit `nosd` fechado (partilha o cgroup do serviço —
  o alias certo deve resolver o health `engine.gpu` também).
