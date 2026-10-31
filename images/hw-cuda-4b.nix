# images/hw-cuda-4b.nix — NVIDIA + **só** Qwen3.5-4B-Q6_K na dGPU.
#
# Diferença vs hw-cuda.nix: sem S2 em :8082, sem 0.8B. O nosd gere um
# único llama-server CUDA (S1/managed) com o 4B. Cage continua na iGPU.
{ config, lib, pkgs, self, ... }:
let
  nvidiaBinFixed = pkgs.runCommand "nvidia-bin-elf-pf-w" {
    nativeBuildInputs = [ pkgs.python3 ];
    src = config.hardware.nvidia.package.bin;
  } ''
    mkdir -p "$out/bin"
    cp -a "$src/bin/." "$out/bin/"
    chmod -R u+w "$out/bin"
    for b in nvidia-smi nvidia-powerd nvidia-cuda-mps-control; do
      if [ -f "$out/bin/$b" ]; then
        ${pkgs.python3}/bin/python3 ${../scripts/fix-nvidia-elf-pf-w.py} "$out/bin/$b"
      fi
    done
    ${pkgs.python3}/bin/python3 - "$out/bin/nvidia-smi" <<'PY'
import struct, pathlib, sys
p = pathlib.Path(sys.argv[1])
b = p.read_bytes()
e_phoff = struct.unpack_from("<Q", b, 32)[0]
e_phentsize, e_phnum = struct.unpack_from("<HH", b, 54)
dyn = None
for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    t, flags, _o, vaddr, _p, _f, memsz, _a = struct.unpack_from("<IIQQQQQQ", b, off)
    if t == 2:
        dyn = vaddr
        break
assert dyn is not None, "no PT_DYNAMIC"
ok = False
for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    t, flags, _o, vaddr, _p, _f, memsz, _a = struct.unpack_from("<IIQQQQQQ", b, off)
    if t == 1 and vaddr <= dyn < vaddr + memsz:
        assert flags & 2, f"PT_LOAD still RO flags={flags:#x}"
        ok = True
assert ok, "no LOAD covering DYNAMIC"
print("nvidia-smi PF_W ok", file=sys.stderr)
PY
  '';

  waitNvidia = pkgs.writeShellScript "nosd-wait-nvidia" ''
    set -eu
    for i in $(seq 1 40); do
      if [ -e /dev/nvidia0 ] || [ -d /proc/driver/nvidia/gpus ]; then
        exit 0
      fi
      sleep 0.25
    done
    echo "nosd-wait-nvidia: timeout (continuando)" >&2
    exit 0
  '';

  modelPath = "/var/lib/nosd/models/active.gguf";
in
{
  hardware.nvidia = {
    modesetting.enable = true;
    open = false;
    powerManagement.enable = false;
    nvidiaSettings = false;
  };
  services.xserver.videoDrivers = lib.mkForce [ "nvidia" ];
  boot.kernelParams = lib.mkAfter [ "nvidia-drm.modeset=1" ];

  systemd.services.nosd.path = lib.mkBefore [
    self.packages.${pkgs.system}.llama-cpp-cuda
  ];
  systemd.services.nosd.serviceConfig.ExecStartPre = lib.mkAfter [
    "+${waitNvidia}"
  ];
  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "CUDA_VISIBLE_DEVICES=0"
    "NVIDIA_VISIBLE_DEVICES=0"
    "NOSD_LLAMA_NGL=99"
    "NOSD_LLAMA_MODEL=${modelPath}"
    # Sem S2: não definir NOSD_LLAMA_SERVER_S2 / MODEL_S2.
    "NOSD_NVIDIA_SMI=${nvidiaBinFixed}/bin/nvidia-smi"
  ];
  systemd.services.nosd.serviceConfig.DeviceAllow = lib.mkAfter [
    "char-nvidia rw"
    "char-nvidia-uvm rw"
    "char-nvidia-caps rw"
  ];

  environment.systemPackages = lib.mkAfter [
    config.hardware.nvidia.package
    nvidiaBinFixed
  ];
}
