# images/hw-cuda.nix — variante do iso-hw com NVIDIA proprietário + CUDA.
#
# Porque existe: o iso-hw base usa o driver **nouveau** (open-source, KMS) —
# dá display mas **não** dá CUDA. Numa RTX 3050 (Ampere) a inferência só
# acelera com o driver proprietário. O cage/JARBAS continua na iGPU (o
# `pickDrm` prefere Intel); o dGPU fica para o llama-server (CUDA).
#
# S1 = 0.8B e S2/PRO = 4B **ambos em dGPU** (`llama-cpp-cuda`, `--fit on`).
# Os dois modelos viajam na imagem (`images/hw.nix` já pina os 5 GGUFs);
# o router decide por pedido.
#
# NUNCA release: unfree + driver + CUDA; é bancada/hw.
{ config, lib, pkgs, self, ... }:
let
  # nvidia-x11-*-bin: patchelf mete .dynamic num PT_LOAD só PF_R → SIGSEGV
  # no glibc 2.44 (DT_DEBUG). Metal 2026-10-31; prova WSL: OR PF_W → --help OK.
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
    # sanity: every PT_LOAD must have PF_W so ld-linux can write DT_DEBUG and
    # other relocation targets without SIGSEGV (glibc 2.44, metal 2026-11-03).
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
all_w = True
for i in range(e_phnum):
    off = e_phoff + i * e_phentsize
    t, flags, _o, vaddr, _p, _f, memsz, _a = struct.unpack_from("<IIQQQQQQ", b, off)
    if t == 1 and memsz > 0:
        if not (flags & 2):
            all_w = False
            print(f"PT_LOAD at vaddr={vaddr:#x} memsz={memsz:#x} lacks PF_W flags={flags:#x}", file=sys.stderr)
assert all_w, "not all PT_LOAD have PF_W"
print("nvidia-smi PF_W on all PT_LOAD ok", file=sys.stderr)
PY
  '';

  waitNvidia = pkgs.writeShellScript "nosd-wait-nvidia" ''
    set -eu
    # Corrida metal: i915 ~7s, nosd ~10s, nvidia-drm card0 ~11s.
    for i in $(seq 1 40); do
      if [ -e /dev/nvidia0 ] || [ -d /proc/driver/nvidia/gpus ]; then
        exit 0
      fi
      sleep 0.25
    done
    echo "nosd-wait-nvidia: timeout (continuando; capacity pode re-probe)" >&2
    exit 0
  '';
in
{
  hardware.nvidia = {
    modesetting.enable = true;
    # Ampere (GA107) com módulos proprietários: caminho CUDA estável.
    open = false;
    powerManagement.enable = false;
    nvidiaSettings = false;
  };
  services.xserver.videoDrivers = lib.mkForce [ "nvidia" ];
  # mkAfter: hw.nix também acrescenta `console=tty0` (não sobrescrever).
  boot.kernelParams = lib.mkAfter [ "nvidia-drm.modeset=1" ];

  # S1 managed: llama-cpp-cuda no PATH (antes do CPU de hw.nix).
  systemd.services.nosd.path = lib.mkBefore [
    self.packages.${pkgs.system}.llama-cpp-cuda
  ];
  systemd.services.nosd.serviceConfig.ExecStartPre = lib.mkAfter [
    "+${waitNvidia}"
  ];
  systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
    "CUDA_VISIBLE_DEVICES=0"
    "NVIDIA_VISIBLE_DEVICES=0"
    # S1: 0.8B na dGPU. Sem NOSD_LLAMA_CTX/NGL fixos: `--fit on` decide
    # camadas+ctx que cabem. NÃO reintroduzir `NOSD_LLAMA_NGL=99`: força o
    # offload no spawn managed e o fit deixa de poder cortar (metal
    # 2026-11-03: S2 morreu com `cudaMalloc: out of memory` a alocar
    # 3352 MiB num GPU de 6 GiB já ocupado; só recuperou depois a NGL 5).
    "NOSD_LLAMA_MODEL=/var/lib/nosd/models/Qwen3.5-0.8B-Q4_K_M.gguf"
    # S2/PRO: 4B Q6_K (tier alpha.2 para 6 GB) no llama-server 8082.
    "NOSD_LLAMA_SERVER_S2=http://127.0.0.1:8082"
    "NOSD_LLAMA_MODEL_S2=/var/lib/nosd/models/Qwen3.5-4B-Q6_K.gguf"
    # Binário com PF_W no PT_LOAD do .dynamic (allowlist D1).
    "NOSD_NVIDIA_SMI=${nvidiaBinFixed}/bin/nvidia-smi"
  ];
  # DeviceAllow=... implica política `closed` (o unit base já tem char-drm,
  # neural-os.nix). Os aliases resolvem por /proc/devices (Linux-only):
  # `char-nvidia` cobre /dev/nvidiaN + /dev/nvidiactl (major 195).
  # `char-nvidia-frontend` só existe com o módulo vGPU/Xen e
  # `char-nvidia-ctl` NÃO é alias — sob closed o systemd ignora-os em
  # silêncio e a CUDA morre a abrir /dev/nvidia0
  # (Permission denied → ENODEV do runtime CUDA). Ver
  # checks.hw-cuda-deviceallow no flake.
  systemd.services.nosd.serviceConfig.DeviceAllow = lib.mkAfter [
    "char-nvidia rw"
    "char-nvidia-uvm rw"
    "char-nvidia-caps rw"
  ];

  # Pacote driver (libs) + binários ELF-fixed (nvidia-smi).
  # `lib.hiPrio` é obrigatório: o nixpkgs entra com `[ nvidia_x11.bin ]`
  # (meta.priority = 4, generic.nix) e o system-path usa
  # `ignoreCollisions = true` — prioridade MENOR ganha a colisão de
  # `bin/nvidia-smi`. Com priority 5 (default) o perfil ficava com o binário
  # por corrigir e o fix de ELF existia só no store: metal 2026-11-03,
  # `hw-info.log` executou `/run/current-system/sw/bin/nvidia-smi` e crashou
  # em 0x401488 (DT_DEBUG).
  environment.systemPackages = lib.mkAfter [
    config.hardware.nvidia.package
    (lib.hiPrio nvidiaBinFixed)
  ];

  # S2/PRO: 4B no dGPU, porta 8082 (loopback). Sem `-c` nem `-ngl` fixos:
  # `--fit on` escolhe camadas+ctx que cabem na VRAM (e o DIAG mostra a VRAM
  # usada — é essa a prova de que o 4B está na GPU).
  systemd.services.nosd-engine-s2 = {
    description = "llama-server S2/PRO (Qwen 4B Q6_K, CUDA)";
    after = [ "local-fs.target" ];
    wantedBy = [ "multi-user.target" ];
    path = [ pkgs.coreutils ];
    startLimitIntervalSec = 120;
    startLimitBurst = 5;
    serviceConfig = {
      Type = "simple";
      User = "nosd";
      Group = "nosd";
      Restart = "on-failure";
      RestartSec = "5s";
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "/var/lib/nosd" ];
      DevicePolicy = "closed";
      # `char-nvidia` cobre /dev/nvidiaN + /dev/nvidiactl (major 195);
      # frontend/ctl não são aliases válidos em metal (ver comentário
      # acima e checks.hw-cuda-deviceallow).
      DeviceAllow = [
        "char-nvidia rw"
        "char-nvidia-uvm rw"
        "char-nvidia-caps rw"
      ];
      SupplementaryGroups = [ "video" "render" ];
      OOMScoreAdjust = 200;
      MemoryMax = "8G";
      Environment = [ "CUDA_VISIBLE_DEVICES=0" ];
      ExecStartPre = "+${waitNvidia}";
      ExecStart = "${self.packages.${pkgs.system}.llama-cpp-cuda}/bin/llama-server --host 127.0.0.1 --port 8082 -m /var/lib/nosd/models/Qwen3.5-4B-Q6_K.gguf --fit on --fit-ctx 2048 --reasoning off --no-webui";
    };
  };
}
