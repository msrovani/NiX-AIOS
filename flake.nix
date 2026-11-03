{
  description = "NixAIOS — nosd daemon + JARBAS kiosk on NixOS (ADR-0118)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    nixos-generators.url = "github:nix-community/nixos-generators";
    nixos-generators.inputs.nixpkgs.follows = "nixpkgs";
    # F0c: pin neural-sgdb (ver docs/NSGDB_VERSION.md)
    neural-sgdb.url = "github:msrovani/neural-sgdb/ce5a5c70e1cfa50eb73af68f36d856c8a6f2bbf1";
    neural-sgdb.flake = false;
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, nixos-generators, neural-sgdb, ... }:
    let
      systems = [ "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
            config.allowUnfree = true;
          };
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" ];
          };
          nosdPkg = pkgs.rustPlatform.buildRustPackage {
            pname = "nosd";
            version = "0.7.0-alpha.5";
            # Sem filtro, `cargo test` no host deixa packages/nosd/target/
            # (GBs) e o Nix copia-o read-only para a store; o build seguinte
            # rebenta com "cp: ... target/...: Read-only file system".
            # F0: inject neural-sgdb pin into vendor/ before cargo build.
            src = pkgs.runCommand "nosd-src" { } ''
              mkdir -p $out
              cp -r ${builtins.path {
                path = ./packages/nosd;
                name = "nosd-tree";
                filter = path: _type:
                  let n = builtins.baseNameOf path; in
                  n != "target" && n != ".direnv" && n != "result" && n != "vendor";
              }}/* $out/
              mkdir -p $out/vendor
              cp -r ${neural-sgdb} $out/vendor/neural-sgdb
            '';
            cargoLock.lockFile = ./packages/nosd/Cargo.lock;
            # Onda 5: JARBAS Slint no binário ISO (cage --ui).
            # nixpkgs 26.11 removeu cargoExtraArgs (era ignorado em silêncio →
            # HUD caía em slog "built without feature slint-ui").
            cargoBuildFlags = [
              "--workspace"
              "--features"
              "nos-jarbas/slint-ui"
            ];
            nativeBuildInputs = with pkgs; [
              pkg-config
              rust
              clang
            ];
            buildInputs = with pkgs; [
              openssl
              libgcc
              fontconfig
              freetype
              libxkbcommon
              wayland
              libGL
              xorg.libX11
              xorg.libXcursor
              xorg.libXrandr
              xorg.libXi
            ] ++ pkgs.lib.optionals pkgs.config.cudaSupport [
              pkgs.cudaPackages.cudatoolkit
            ];
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            # RPATH for future CUDA/ROCm engines under $out/lib/nosd/engines
            postFixup = ''
              mkdir -p $out/lib/nosd/engines
              if [ -d "$out/bin" ]; then
                for b in $out/bin/*; do
                  if [ -f "$b" ]; then
                    patchelf --set-rpath "${pkgs.lib.makeLibraryPath [
                      pkgs.libgcc pkgs.openssl pkgs.fontconfig pkgs.freetype
                      pkgs.libxkbcommon pkgs.wayland pkgs.libGL
                    ]}:$out/lib/nosd/engines" "$b" || true
                  fi
                done
              fi
            '';
            meta = {
              description = "Neural-OS Daemon (NixAIOS)";
              license = pkgs.lib.licenses.agpl3Only;
              mainProgram = "nosd";
            };
          };
        in
        {
          nosd = nosdPkg;
          default = nosdPkg;
          # Default nixpkgs llama-cpp = CPU. CUDA build is heavy / unfree — opt-in:
          #   nix build .#llama-cpp-cuda
          # Then: NOSD_LLAMA_CLI=result/bin/llama-completion NOSD_LLAMA_NGL=99
          # Forward GPU = VERIFIED só com system_info CUDA + offload (não basta nvidia-smi).
          llama-cpp-cuda = pkgs.llama-cpp.override { cudaSupport = true; };

          # Brain embutível na iso-hw (NUNCA na ISO de release, S1.2):
          # HW real já arranca COM o brain — sem download no first boot.
          # GGUF content-addressed (pin por SRI) = a reproducibilidade que o
          # download runtime não dava (G4.1: modelo como derivação Nix).
          # Fontes: S1 = checkpoint editado RikiyaT/akhaliq (verificável bit-
          # a-bit), quantizado pela NotCherry; fallback = unsloth (Apache-2.0).
          # 0.7-alpha1: a iso-hw embute TODOS os tiers pinados abaixo
          # (offline). O default continua 0.8B Q4 (AGORA com edito da Astra);
          # 4B/9B só activam por brain.plan/accept (HITL, ring para o 9B). A
          # ISO de release continua sem modelos — ver
          # docs/adr-addenda/0118-qwen-embedded-brain.md.
          # 2026-11-03: S1 embutido passa a ser o Qwen3.5-0.8B **editado pela
          # Astra** (Weight-Space Programming, MIT CSAIL/RikiyaT 2026-11-02):
          # sem treino, subtractiu uma "direcção de loop" (thinking que nunca
          # acaba) de down_proj das camadas 15/19 — 131 072 pesos (0.016%).
          # Post: MATH-500 7.9→48.1%, GSM8K 13.8→67.1%, IFEval 21.0→62.3%,
          # hit-token-limit 93→33%. Re-run independente (akhaliq, H200):
          # 51.0%. Bit-exact vs base @2fc0636… (2 tensores, sha verificado).
          # Fonte GGUF: NotCherry/…-weight-space-programming-GGUF (Q4_K_M,
          # LFS sha256 0be511c8…, 541903776 B). CAVEATS honestos:
          # (a) ganhos provados com **thinking ON** + contexto longo (32k);
          #     com --reasoning off o ganho pode não se materializar;
          # (b) não há evidência de que o edito sobreviva à quantização
          #     (o repo GGUF não faz claims de qualidade);
          # (c) quantização única Q4_K_M de publisher novo (sem reputação),
          #     baseado no checkpoint re-verificado do RikiyaT/akhaliq.
          # Status: PINADO (hash) — engine/metal OBSERVED pendente.
          qwen35-08b-q4km = pkgs.fetchurl {
            url = "https://huggingface.co/NotCherry/Qwen3.5-0.8B-weight-space-programming-GGUF/resolve/main/Qwen3.5-0.8B-Weight-Space-Programming-Q4_K_M.gguf";
            # LFS sha256 do HF tree (NotCherry/…-GGUF), 541903776 B.
            hash = "sha256-C+URyDJQVeE1nLZC20LaNPLcEGmRKP1LZJZ4FeP/ahs=";
          };
          # Rollback: original **sem edito** da unsloth, mantido para
          # revert/AB (o veredito do edito em quantizado não é VERIFIED).
          qwen35-08b-q4km-base = pkgs.fetchurl {
            url = "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q4_K_M.gguf";
            # LFS sha256 do HF tree (unsloth/Qwen3.5-0.8B-GGUF), 532517120 B.
            hash = "sha256-vSWHguNff0WPis7RrcBT5ukuibxzW6O+idOKBhIdxRc=";
          };
          # Tier upgrade proposto pós-boot quando o hw permitir (RAM/NVRAM).
          # NOTA: ainda aponta para o base sem edito (Q8_0 da unsloth não
          # existe para o checkpoint editado) — trocar quando converterem.
          qwen35-08b-q8 = pkgs.fetchurl {
            url = "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q8_0.gguf";
            hash = "sha256-CtiF/9S7Ai/E8NM6Mwj6EI74YTFZ07OmfiOrygVremw=";
          };
          # Tier 3 pinado em nos-hermes::brain_upgrade (4B Q4_K_M, 2 740 937 888 B).
          # Para a variante CUDA (3050 6 GB); NÃO na release.
          qwen35-4b-q4km = pkgs.fetchurl {
            url = "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf";
            hash = "sha256-AP55hv9fa0Y+YkVYIRRgSdtvkxNgOTinCADR+2nvEaQ=";
          };
          # 0.7-alpha2: 4B Q6_K (3 525 956 768 B) para o perfil 6 GB —
          # mais precisão que Q4; contexto máximo via llama-server --fit.
          qwen35-4b-q6k = pkgs.fetchurl {
            url = "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q6_K.gguf";
            hash = "sha256-/e3XgcnOZ2q2awGMokf/eOijPJgJioIsHi1Qdedxj2Y=";
          };
          # Tier 4 pinado em brain_upgrade (9B Q4_K_M, 5 680 522 464 B):
          # incluído na iso-hw 0.7-alpha1; activação exige ring ARMADO.
          qwen35-9b-q4km = pkgs.fetchurl {
            url = "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf";
            hash = "sha256-A7dHJ6hgpWM44ELEQguz8Esv7Fc0F19MufqFPa9St+g=";
          };
          # STT offline (whisper.cpp): sem isto o cinematic entra em
          # listening e falha no voice.stt (ModelAbsent ggml-base).
          ggml-tiny = pkgs.fetchurl {
            url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin";
            hash = "sha256-vgfgSOHlma1GNByNKhNWRQl6U4IhZ4t6zdGxkZxuGyE=";
          };
          # TTS offline (piper): voz pt_BR-faber-medium; binário = pkgs.piper-tts.
          # Sem onnx o nos-voice cai em espeak-ng (robótico) ou EngineAbsent.
          piper-pt-br-faber-medium-onnx = pkgs.fetchurl {
            url = "https://huggingface.co/rhasspy/piper-voices/resolve/main/pt/pt_BR/faber/medium/pt_BR-faber-medium.onnx";
            hash = "sha256-hYVV46BkIJxXCI/mvXDEw9xU0D6qAMRdXsr0OjP5Wqc=";
          };
          piper-pt-br-faber-medium-json = pkgs.fetchurl {
            url = "https://huggingface.co/rhasspy/piper-voices/resolve/main/pt/pt_BR/faber/medium/pt_BR-faber-medium.onnx.json";
            hash = "sha256-fmlN4ZWuP8Nt1zLEResE+0m2SYVIk8tVBrl48NUKHW8=";
          };
          # Binário rhasspy/piper release (libs+espeak-data). Evita pkgs.piper-tts
          # (dep pyopenjtalk-plus falha no check em nixpkgs actual).
          piper-standalone = pkgs.stdenvNoCC.mkDerivation {
            pname = "piper-standalone";
            version = "2023.11.14-2";
            src = pkgs.fetchurl {
              url = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz";
              hash = "sha256-pQy0XzVbevH211jBs2BxeHe6CjmMyMvm0qejom4iWZI=";
            };
            dontConfigure = true;
            dontBuild = true;
            dontFixup = true;
            installPhase = ''
              mkdir -p $out
              # unpackPhase já entra em sourceRoot=piper/; conteúdo → $out
              # (L+ /var/lib/nosd/models/piper → $out/piper)
              cp -a . "$out/"
            '';
          };
        } // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") {
          # Convenience: nix build .#iso (from WSL/Linux)
          iso = self.nixosConfigurations.nixaios-iso.config.system.build.isoImage;
          # Demo (NUNCA release): mesh pre-configurada + ssh para assercoes.
          iso-demo =
            self.nixosConfigurations.nixaios-iso-demo.config.system.build.isoImage;
          # HW (nao release): logs persistentes em FAT32 LABEL=NIXAIOSLOG.
          iso-hw =
            self.nixosConfigurations.nixaios-iso-hw.config.system.build.isoImage;
          # Variante CUDA (RTX 3050): NVIDIA proprietario + llama-cpp-cuda.
          iso-hw-cuda =
            self.nixosConfigurations.nixaios-iso-hw-cuda.config.system.build.isoImage;
          # HW CUDA só com Qwen3.5-4B-Q6_K (sem 0.8B/9B/S2).
          iso-hw-cuda-4b =
            self.nixosConfigurations.nixaios-iso-hw-cuda-4b.config.system.build.isoImage;
          # Opcional Pascal (GTX 1050): driver legacy.
          iso-hw-cuda-legacy =
            self.nixosConfigurations.nixaios-iso-hw-cuda-legacy.config.system.build.isoImage;
        });

      nixosModules.neural-os = import ./modules/neural-os.nix;
      nixosModules.jarbas-cage = import ./modules/jarbas-cage.nix;
      nixosModules.ai-managed = import ./modules/ai-managed.nix;
      nixosModules.log-fat32 = import ./modules/log-fat32.nix;
      nixosModules.cortex-nix-ring = import ./modules/cortex-nix-ring.nix;
      nixosModules.nosd-engine = import ./modules/nosd-engine.nix;
      nixosModules.nosd-early = import ./modules/nosd-early.nix;
      nixosModules.transmute-stage = import ./modules/transmute-stage.nix;
      nixosModules.aios-host-perf = import ./modules/aios-host-perf.nix;
      nixosModules.nosd-brain = import ./modules/nosd-brain.nix;
      nixosModules.nosd-tts = import ./modules/nosd-tts.nix;
      nixosModules.m3b-metal-probe = import ./modules/m3b-metal-probe.nix;
      nixosModules.default = { ... }: {
        imports = [
          self.nixosModules.neural-os
          self.nixosModules.jarbas-cage
          self.nixosModules.ai-managed
          self.nixosModules.log-fat32
          self.nixosModules.cortex-nix-ring
          self.nixosModules.nosd-engine
          self.nixosModules.nosd-brain
          self.nixosModules.nosd-tts
          self.nixosModules.nosd-early
          self.nixosModules.transmute-stage
          self.nixosModules.aios-host-perf
          self.nixosModules.m3b-metal-probe
        ];
      };

      checks = forAllSystems (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
            config.allowUnfree = true;
          };
          # Fixture de teste (não entra na ISO): tiny GGUF CPU.
          tinyGguf = pkgs.fetchurl {
            url = "https://huggingface.co/ggml-org/models/resolve/main/tinyllamas/stories15M-q4_0.gguf";
            hash = "sha256-ZpZ/vs5tvpeIZZP9u3NYlYSSfikRnsMfCAkHMtGGFzk=";
          };
          tinyWhisper = pkgs.fetchurl {
            url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin";
            hash = "sha256-vgfgSOHlma1GNByNKhNWRQl6U4IhZ4t6zdGxkZxuGyE=";
          };
          jfkWav = pkgs.fetchurl {
            url = "https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/samples/jfk.wav";
            hash = "sha256-Wd+5pKyzb+Kir/wUusvuKSD/Q1yxPMMUoIwT9munhg4=";
          };
          # M5.1: no base do check de 3 VMs com falhas (latencia/perda/kill).
          meshNode3 = ip: { lib, ... }: {
            imports = [ self.nixosModules.neural-os ];
            virtualisation.vlans = [ 1 ];
            networking.interfaces.eth1.ipv4.addresses = [
              { address = ip; prefixLength = 24; }
            ];
            networking.firewall.enable = false;
            # sch_netem: o teste injeta latencia/perda com tc.
            boot.kernelModules = [ "sch_netem" ];
            services.nosd = {
              enable = true;
              package = self.packages.${system}.nosd;
              hermesTickSecs = 1;
              meshBind = "0.0.0.0:19000";
              meshBroadcast = "192.168.50.255:19000";
            };
            systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
              "NOSD_MESH_REQUIRE_SIG=1"
              "NOSD_JOB_BIND=0.0.0.0:19001"
            ];
            environment.systemPackages = [
              self.packages.${system}.nosd
              pkgs.iproute2
            ];
            system.stateVersion = "25.11";
          };
        in
        {
          # Seed offline NIXAIOSLOG/models -> /var/lib/nosd/models (script puro).
          seed-models = pkgs.runCommand "test-seed-models" {
            nativeBuildInputs = with pkgs; [
              bash
              coreutils
              gawk
              gnugrep
              diffutils
            ];
            SEED = ./modules/transmute/seed-models.sh;
          } ''
            bash ${./scripts/test-seed-models.sh}
            touch $out
          '';

          # Wi-Fi por wifi.conf (nmcli fake; sem NM real).
          seed-wifi = pkgs.runCommand "test-seed-wifi" {
            nativeBuildInputs = with pkgs; [
              bash
              coreutils
              gnugrep
            ];
            SEED = ./modules/transmute/seed-wifi.sh;
          } ''
            bash ${./scripts/test-seed-wifi.sh}
            touch $out
          '';

          # Persistencia das respostas do firstboot no pendrive.
          stick-conf = pkgs.runCommand "test-stick-conf" {
            nativeBuildInputs = with pkgs; [
              bash
              coreutils
              gnugrep
            ];
            HELPERS = ./modules/transmute/stick-conf.sh;
          } ''
            bash ${./scripts/test-stick-conf.sh}
            touch $out
          '';

          # nvidia-smi ELF: PT_LOAD que cobre PT_DYNAMIC deve ter PF_W
          # (glibc 2.44 DT_DEBUG). Usa o binário apontado por NOSD_NVIDIA_SMI
          # na config iso-hw-cuda (derivação nvidia-bin-elf-pf-w).
          nvidia-smi-elf-pf-w =
            let
              cfg = self.nixosConfigurations.nixaios-iso-hw-cuda.config;
              envList = cfg.systemd.services.nosd.serviceConfig.Environment;
              envs = builtins.filter builtins.isString
                (nixpkgs.lib.flatten
                  (if builtins.isList envList then envList else [ envList ]));
              smiLine = nixpkgs.lib.findFirst
                (s: nixpkgs.lib.hasPrefix "NOSD_NVIDIA_SMI=" s)
                null
                envs;
              smiPath = if smiLine == null then null
                else nixpkgs.lib.removePrefix "NOSD_NVIDIA_SMI=" smiLine;
            in
            pkgs.runCommand "test-nvidia-smi-elf-pf-w" {
              nativeBuildInputs = [ pkgs.python3 ];
              inherit smiPath;
            } ''
              if [ -z "$smiPath" ] || [ ! -f "$smiPath" ]; then
                echo "NOSD_NVIDIA_SMI missing or not a file: $smiPath" >&2
                exit 1
              fi
              ${pkgs.python3}/bin/python3 - "$smiPath" <<'PY'
import struct, pathlib, sys
p = pathlib.Path(sys.argv[1])
b = p.read_bytes()
assert b[:4] == b"\x7fELF", "not ELF"
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
        assert flags & 2, f"PT_LOAD covering .dynamic lacks PF_W flags={flags:#x}"
        ok = True
assert ok, "no PT_LOAD covers PT_DYNAMIC"
print(f"ok {p} PF_W on DYNAMIC LOAD", file=sys.stderr)
PY
              touch $out
            '';

          # hw-cuda (metal/RTX 3050): DeviceAllow resolvem aliases por
          # /proc/devices (Linux-only, enforcement systemd/cgroup). `char-
          # nvidia` cobre /dev/nvidiaN + /dev/nvidiactl (major 195);
          # `char-nvidia-frontend` só existe com o módulo vGPU/Xen e
          # `char-nvidia-ctl` não é alias — sob política `closed` implícita o
          # systemd ignora-os em silêncio e a CUDA morre a abrir
          # /dev/nvidia0 (Permission denied → ENODEV). Check estático (eval
          # da config + grep; NÃO prova CUDA — metal continua AWAITING_HW).
          hw-cuda-deviceallow =
            let
              cfg = self.nixosConfigurations.nixaios-iso-hw-cuda.config;
            in
            pkgs.runCommand "test-hw-cuda-deviceallow" {
              nativeBuildInputs = with pkgs; [ gnugrep ];
              NOSD_DA = builtins.toJSON cfg.systemd.services.nosd.serviceConfig.DeviceAllow;
              S2_DA = builtins.toJSON cfg.systemd.services."nosd-engine-s2".serviceConfig.DeviceAllow;
              # Unit do próprio módulo (não activo no hw-cuda: eval mínimo
              # com supervise=true) — a 1ª versão do fix esqueceu este unit.
              ENGINE_DA = builtins.toJSON
                (nixpkgs.lib.nixosSystem {
                  system = "x86_64-linux";
                  modules = [
                    self.nixosModules.nosd-engine
                    (_: {
                      services.nosdEngine = {
                        enable = true;
                        supervise = true;
                        modelPath = "/var/lib/nosd/models/fake.gguf";
                      };
                    })
                  ];
                }).config.systemd.services."nosd-engine".serviceConfig.DeviceAllow;
            } ''
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-nvidia rw"' <<<"$NOSD_DA"; then
                echo "missing char-nvidia rw in nosd DeviceAllow: $NOSD_DA" >&2; exit 1
              fi
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-nvidia rw"' <<<"$S2_DA"; then
                echo "missing char-nvidia rw in nosd-engine-s2 DeviceAllow: $S2_DA" >&2; exit 1
              fi
              if ${pkgs.gnugrep}/bin/grep -Eq 'char-nvidia-(frontend|ctl)' <<<"$NOSD_DA"; then
                echo "dead alias in nosd DeviceAllow: $NOSD_DA" >&2; exit 1
              fi
              if ${pkgs.gnugrep}/bin/grep -Eq 'char-nvidia-(frontend|ctl)' <<<"$S2_DA"; then
                echo "dead alias in nosd-engine-s2 DeviceAllow: $S2_DA" >&2; exit 1
              fi
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-nvidia rw"' <<<"$ENGINE_DA"; then
                echo "missing char-nvidia rw in nosd-engine DeviceAllow: $ENGINE_DA" >&2; exit 1
              fi
              if ${pkgs.gnugrep}/bin/grep -Eq 'char-nvidia-(frontend|ctl)' <<<"$ENGINE_DA"; then
                echo "dead alias in nosd-engine DeviceAllow: $ENGINE_DA" >&2; exit 1
              fi
              echo "nosd DeviceAllow (visible): $NOSD_DA" >&2
              touch $out
            '';

          # Voz no metal: `nos-voice` corre `arecord`/`aplay` como filhos do
          # nosd, que tem DeviceAllow (→ DevicePolicy=closed). Sem `char-alsa`
          # (/dev/snd, major 116) o cgroup de dispositivos nega a captura e o
          # sintoma parece “microfone avariado”. Gate estático, não prova áudio.
          nosd-voice-deviceallow =
            let
              cfg = nixpkgs.lib.nixosSystem {
                system = "x86_64-linux";
                modules = [
                  self.nixosModules.neural-os
                  (_: { services.nosd.enable = true; })
                ];
              };
            in
            pkgs.runCommand "test-nosd-voice-deviceallow" {
              nativeBuildInputs = [ pkgs.gnugrep ];
              DA = builtins.toJSON cfg.config.systemd.services.nosd.serviceConfig.DeviceAllow;
              ENV = builtins.toJSON cfg.config.systemd.services.nosd.serviceConfig.Environment;
              PRE = builtins.toJSON cfg.config.systemd.services.nosd.serviceConfig.ExecStartPre;
            } ''
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-alsa rw"' <<<"$DA"; then
                echo "missing char-alsa rw in nosd DeviceAllow (arecord/aplay): $DA" >&2; exit 1
              fi
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-drm rw"' <<<"$DA"; then
                echo "missing char-drm rw in nosd DeviceAllow: $DA" >&2; exit 1
              fi
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"XDG_RUNTIME_DIR=/run/nosd/user"' <<<"$ENV"; then
                echo "missing XDG_RUNTIME_DIR=/run/nosd/user in nosd Environment: $ENV" >&2; exit 1
              fi
              if ! ${pkgs.gnugrep}/bin/grep -Fq 'install -d -m 0700 -o nosd -g nosd /run/nosd/user' <<<"$PRE"; then
                echo "missing root install of /run/nosd/user in ExecStartPre: $PRE" >&2; exit 1
              fi
              touch $out
            '';

          # Cinematic: Chromium/PipeWire correm no unit jarbas-cage. Sem
          # char-alsa o getUserMedia e o WebAudio (TTS no browser) morrem
          # com DevicePolicy=closed — sintoma "MIC OFF / sem som".
          jarbas-cage-alsa-deviceallow =
            let
              cfg = nixpkgs.lib.nixosSystem {
                system = "x86_64-linux";
                modules = [
                  self.nixosModules.neural-os
                  self.nixosModules.jarbas-cage
                  (_: {
                    services.nosd.enable = true;
                    services.jarbasCage = {
                      enable = true;
                      uiMode = "cinematic";
                      autoStart = false;
                    };
                  })
                ];
              };
            in
            pkgs.runCommand "test-jarbas-cage-alsa-deviceallow" {
              nativeBuildInputs = [ pkgs.gnugrep ];
              DA = builtins.toJSON cfg.config.systemd.services.jarbas-cage.serviceConfig.DeviceAllow;
            } ''
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-alsa rw"' <<<"$DA"; then
                echo "missing char-alsa rw in jarbas-cage DeviceAllow (mic/TTS browser): $DA" >&2; exit 1
              fi
              if ! ${pkgs.gnugrep}/bin/grep -Fq '"char-video4linux rw"' <<<"$DA"; then
                echo "missing char-video4linux rw in jarbas-cage DeviceAllow: $DA" >&2; exit 1
              fi
              touch $out
            '';

          # O motor supervisionado tem de usar as MESMAS flags do spawn do
          # nosd: `--fit on`/`--fit-ctx` (contexto conforme VRAM) e
          # `--reasoning off`. Sem isto, `supervise=true` era outro motor.
          engine-fit-parity =
            let
              cfg = nixpkgs.lib.nixosSystem {
                system = "x86_64-linux";
                modules = [
                  self.nixosModules.nosd-engine
                  (_: {
                    services.nosdEngine = {
                      enable = true;
                      supervise = true;
                      modelPath = "/var/lib/nosd/models/fake.gguf";
                    };
                  })
                ];
              };
              svc = cfg.config.systemd.services;
            in
            pkgs.runCommand "test-engine-fit-parity" {
              nativeBuildInputs = [ pkgs.gnugrep ];
              SUPERVISED = svc."nosd-engine".serviceConfig.ExecStart;
              TEMPLATE = svc."nosd-engine@".serviceConfig.ExecStart;
            } ''
              # Unit concreto: as flags estão no ExecStart.
              for flag in '--fit on' '--fit-ctx 2048' '--reasoning off' '--no-webui'; do
                if ! ${pkgs.gnugrep}/bin/grep -Fq -- "$flag" <<<"$SUPERVISED"; then
                  echo "supervised sem '$flag' (paridade com nos-cortex): $SUPERVISED" >&2; exit 1
                fi
              done
              # Template: o ExecStart é o script; as flags vivem dentro dele.
              TEMPLATE_SCRIPT="''${TEMPLATE%% *}"
              for flag in '--fit on' '--fit-ctx 2048' '--reasoning off' '--no-webui'; do
                if ! ${pkgs.gnugrep}/bin/grep -Fq -- "$flag" "$TEMPLATE_SCRIPT"; then
                  echo "template sem '$flag' em $TEMPLATE_SCRIPT" >&2; exit 1
                fi
              done
              touch $out
            '';

          # P0 #2 (revisão simbiose 2026-10-31): o aceite antigo — "matar o
          # processo e ver o systemd reiniciar" — provava `Restart=`, não
          # `WatchdogSec=`. Um processo morto sai; o watchdog existe para o
          # processo PENDURADO, que não sai e não notifica. Aqui o `nosd` é
          # `SIGSTOP`ado: só o prazo do watchdog (30 s) o reinicia, e o
          # `TimeoutStopSec` curto existe porque um processo parado não apanha
          # o SIGABRT antes do SIGKILL — sem isso a prova levaria 90 s.
          nosd-watchdog = pkgs.testers.nixosTest {
            name = "nosd-watchdog";
            nodes.machine = { lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 0;
              };
              systemd.services.nosd.serviceConfig.TimeoutStopSec = "5s";
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              machine.start()
              machine.wait_for_unit("nosd.service")
              machine.wait_for_file("/run/nosd/nosd.sock")
              nosctl = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"

              # Saudável: nenhum reinício. Se o ticker não corresse, o
              # `WatchdogSec=30s` já o teria matado antes deste ponto.
              machine.succeed("test \"$(systemctl show -p NRestarts --value nosd.service)\" = 0")
              machine.succeed(nosctl + " health")

              pid = machine.succeed("systemctl show -p MainPID --value nosd.service").strip()
              machine.succeed(f"kill -STOP {pid}")

              # Pendurado: sem WATCHDOG=1 o systemd aborta e reinicia.
              machine.wait_until_succeeds(
                  "test \"$(systemctl show -p NRestarts --value nosd.service)\" -ge 1",
                  timeout=120,
              )
              machine.wait_until_succeeds("systemctl is-active nosd.service | grep -qx active")

              # Não é laço: UM reinício do watchdog, não um ciclo.
              machine.sleep(60)
              machine.succeed("test \"$(systemctl show -p NRestarts --value nosd.service)\" -le 1")
              machine.succeed(nosctl + " health")
            '';
          };

          nixaios-vm = pkgs.testers.nixosTest {
            name = "nixaios-vm";
            nodes.machine = { lib, ... }: {
              imports = [
                self.nixosModules.neural-os
                self.nixosModules.cortex-nix-ring
              ];
              # S-06/R-04: ring module presente mas DESARMADO (armRing=false).
              services.nosdNixRing = {
                enable = true;
                autoSwitch = false;
              };
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 0;
              };
              # JFK é EN; fixa o locale de voz no teste.
              # alpha.5: o prompt com SOUL completo (~467 tokens) excede o
              # n_ctx_train=128 do tiny GGUF do fixture (llama.cpp capa o slot
              # a 128). Persona vazia aqui; a persona é coberta pelos unit
              # tests do cognitive_bridge. Nao aumentar a memoria da VM: o
              # limite é o modelo, nao a RAM.
              environment.etc."nixaios-empty-soul.md".text = "";
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_LOCALE=en_US.UTF-8"
                "NOSD_SOUL_PATH=/etc/nixaios-empty-soul.md"
                # G1 (T1.1-T1.3): identidade do mesh no VM — âncora CRDT
                # assinada + pins/seq do HB persistidos ao lado da chave.
                "NOSD_MESH_KEY=/var/lib/nosd/keys/mesh.key"
                "NOSD_MESH_REQUIRE_SIG=1"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              environment.etc."nixaios-accept-m3.sh".source = ./scripts/accept-m3.sh;
              # 0.6: tiny GGUF em /var/lib/nosd/models; nosd faz spawn de llama-server.
              system.activationScripts.nixaiosTinyModel = ''
                mkdir -p /var/lib/nosd/models
                cp ${tinyGguf} /var/lib/nosd/models/stories15M-q4_0.gguf
                cp ${tinyWhisper} /var/lib/nosd/models/ggml-tiny.bin
                cp ${jfkWav} /var/lib/nosd/models/test-jfk.wav
                chown -R nosd:nosd /var/lib/nosd/models
                chmod 0755 /var/lib/nosd/models
                chmod 0644 /var/lib/nosd/models/stories15M-q4_0.gguf
                chmod 0644 /var/lib/nosd/models/ggml-tiny.bin
                chmod 0644 /var/lib/nosd/models/test-jfk.wav
              '';
              system.stateVersion = "25.11";
            };
            testScript = ''
              machine.wait_for_unit("nosd.service")
              machine.wait_for_file("/run/nosd/nosd.sock")
              nosctl = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              machine.succeed(nosctl + " health")
              # S-09: socket IPC 0660 (dono nosd:grupo nosd).
              machine.succeed("test \"$(stat -c %a /run/nosd/nosd.sock)\" = 660")
              machine.succeed(nosctl + " health | grep -q 'llama-server-http'")
              machine.succeed(nosctl + " chat 'Once upon a time' | grep -q '\"text\"'")
              machine.succeed(nosctl + " unload")
              machine.succeed(nosctl + " health | grep -q 'cpu-echo'")
              machine.succeed(nosctl + " load /var/lib/nosd/models/stories15M-q4_0.gguf")
              machine.succeed(nosctl + " health | grep -q 'llama-server-http'")
              machine.succeed("bash /etc/nixaios-accept-m3.sh")
              machine.succeed(nosctl + " unload")
              machine.succeed(nosctl + " voice-status | grep -q Ready")
              # voice-stt real (whisper tiny) é manual/QEMU: ~23 min em TCG.
              machine.succeed(nosctl + " voice-tts --no-play hello > /tmp/voice-tts.json")
              machine.succeed("grep -q wav_b64 /tmp/voice-tts.json")
              # S-06/R-04: com o ring desarmado (default), um pedido de apply
              # posto por qualquer user em /var/lib/nosd (1777 na ISO) tem de
              # ser ignorado: nada escrito em /etc/nixos e pedido removido.
              machine.succeed("echo 'staging=/var/lib/nosd/pwn.nix' > /var/lib/nosd/nix-apply.request")
              machine.succeed("systemctl start nosd-nix-apply.service || true")
              machine.succeed("test ! -e /etc/nixos/ai-managed.nix")
              machine.succeed("test ! -e /var/lib/nosd/nix-apply.request")

              # G1 (T1.1-T1.3): com identidade, a ancora do log CRDT fica
              # ASSINADA (3 campos) e os artefactos de persistência sobrevivem
              # ao restart — o reopen com ancora assinada tem de continuar
              # verde (fail-closed correcta, nao falso rollback).
              machine.succeed("systemctl restart nosd.service")
              machine.wait_for_file("/run/nosd/nosd.sock")
              machine.succeed(nosctl + " health")
              anchor = "/var/lib/nosd/mesh-crdt/seg.anchor"
              machine.wait_for_file(anchor)
              machine.succeed("test \"$(wc -w < " + anchor + ")\" = 3")
              machine.succeed("test -e /var/lib/nosd/keys/pins.dat")
              machine.succeed("test -s /var/lib/nosd/keys/hb.seq")
            '';
          };

          # T3.3: helper root two-phase (G3.2) com o ring ARMADO neste node.
          # `nixos-rebuild` é STUB: o alvo é o protocolo do helper — fase 1
          # (copy + test, boot intacta), espera de `nix.confirm` (TTL 600 s),
          # recusa de confirm expirado, fase 2 (switch) e gravação de
          # geração. O rebuild real dentro de VM tester não é viável (TCG +
          # sem nixpkgs resolúvel) e não é o alvo.
          nixaios-ring = pkgs.testers.nixosTest {
            name = "nixaios-ring";
            nodes.machine = { lib, pkgs, config, ... }: {
              imports = [
                self.nixosModules.neural-os
                self.nixosModules.cortex-nix-ring
              ];
              services.nosdNixRing = {
                enable = true;
                autoSwitch = false;
                # G3.3: SÓ neste node de teste (o nixaios-vm base continua
                # provando o desarmado por omissão).
                armRing = true;
                buildVmFirst = false;
                twoPhase = true;
              };
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 0;
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_LOCALE=en_US.UTF-8"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              # T3.3: stub — `nixos-rebuild` loga o argv e sai 0; fase 1 e
              # fase 2 distinguem-se pelo 1º argumento (test vs switch).
              systemd.services.nosd-nix-apply.path = lib.mkForce [
                (pkgs.writeShellScriptBin "nixos-rebuild" ''
                  echo "rebuild $*" >> /var/lib/nosd/rebuild-stub.log
                  exit 0
                '')
                pkgs.nix
                pkgs.util-linux
                pkgs.coreutils
                pkgs.gnugrep
              ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              machine.wait_for_unit("nosd.service")
              machine.wait_for_file("/run/nosd/nosd.sock")
              nosctl = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              machine.succeed(nosctl + " health")
              LOG = "/var/lib/nosd/nix-apply.log"
              STUB = "/var/lib/nosd/rebuild-stub.log"

              # 1) ring ARMADO com pedido forjado sem staging válido: recusa.
              machine.succeed("echo 'staging=/var/lib/nosd/pwn.nix' > /var/lib/nosd/nix-apply.request")
              machine.succeed("systemctl start nosd-nix-apply.service || true")
              machine.succeed("test ! -e /etc/nixos/ai-managed.nix")
              machine.succeed("test ! -e /var/lib/nosd/nix-apply.request")

              # 2) T3.1: propose por FIELDS tipados (template fixo, sem texto livre).
              machine.succeed(nosctl + " nix-propose --fields '{\"hostname\":\"ring-vm\"}'")
              machine.succeed("grep -q 'networking.hostName' /var/lib/nosd/staging-ai-managed.nix")

              # 3) HITL grant (kind=nix.apply) + fase 1: copy + test.
              #    Nada de `switch` no stub e a geraçao de boot fica intacta.
              #    (to_string_pretty imprime `"confirm_id": "..."` — split
              #    tolera o espaço após o ':'.)
              cid_out = machine.succeed(nosctl + " consent nix.apply")
              cid = cid_out.split('"confirm_id":')[1].split('"')[1]
              machine.succeed(nosctl + " nix-apply --confirm-id " + cid)
              machine.wait_until_succeeds("grep -q 'aguarda nix.confirm' " + LOG)
              machine.succeed("test $(grep -c 'rebuild test' " + STUB + ") -eq 1")
              machine.succeed("! grep -q 'rebuild switch' " + STUB)
              machine.succeed("test ! -e /var/lib/nosd/nix-apply.request")
              # fase 1 copia o staging para o target fixo.
              machine.succeed("grep -q 'networking.hostName' /etc/nixos/ai-managed.nix")

              # 4) confirm EXPIRADO (ts antigo): fase 2 recusa (TTL 600 s).
              machine.succeed("echo 'ts=1' > /var/lib/nosd/nix-confirm.request")
              machine.succeed("echo 'staging=/var/lib/nosd/staging-ai-managed.nix' > /var/lib/nosd/nix-apply.request")
              machine.succeed("systemctl start nosd-nix-apply.service || true")
              machine.wait_until_succeeds("grep -q 'FAIL confirm expirado' " + LOG)
              machine.succeed("! grep -q 'rebuild switch' " + STUB)
              machine.succeed("test ! -e /var/lib/nosd/nix-confirm.request")
              machine.succeed("test ! -e /var/lib/nosd/nix-apply.request")

              # 5) fase 2 por RPC: consent → nix.confirm (grava confirm com
              #    ts fresco) → switch no stub + geração gravada.
              cid_out = machine.succeed(nosctl + " consent nix.apply")
              cid2 = cid_out.split('"confirm_id":')[1].split('"')[1]
              machine.succeed(nosctl + " nix-confirm --confirm-id " + cid2)
              machine.wait_until_succeeds("grep -q 'rebuild switch' " + STUB)
              machine.wait_until_succeeds("grep -q 'switch OK' " + LOG)
              # Diagnóstico visível no log do teste (antes do assert).
              machine.execute("cat " + LOG + " || true")
              machine.execute("cat /var/lib/nosd/last-generation.txt 2>&1 || true")
              machine.succeed("test -s /var/lib/nosd/last-generation.txt")
            '';
          };

          # M6: mesh com identidade Ed25519 — HB assinado, pinagem TOFU,
          # anti-replay e recusa de HB sem assinatura (NOSD_MESH_REQUIRE_SIG=1).
          mesh-2vm-signed = pkgs.testers.nixosTest {
            name = "nixaios-mesh-signed";
            nodes.a = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.60.1"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.60.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            nodes.b = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.60.2"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.60.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              start_all()
              a.wait_for_unit("nosd.service")
              b.wait_for_unit("nosd.service")
              a.wait_for_file("/run/nosd/nosd.sock")
              b.wait_for_file("/run/nosd/nosd.sock")
              ca = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              # HB assinado aceite + chave fixada (TOFU)
              a.wait_until_succeeds(ca + " health | grep -q '\"mesh_peers\": 1'", timeout=90)
              b.wait_until_succeeds(ca + " health | grep -q '\"mesh_peers\": 1'", timeout=90)
              a.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              b.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              # HB antigo/sem assinatura (id 99) nao pode entrar
              a.succeed("printf 'HB\\0\\x63\\0\\0\\0PK\\0CAP\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0\\0' > /dev/udp/192.168.60.2/19000 || true")
              import time; time.sleep(3)
              b.succeed(ca + " health | grep -q '\"mesh_peers\": 1'")
              b.succeed("journalctl -u nosd -o cat | grep -q 'sem assinatura'")
            '';
          };

          # M9.1: SKILL_PUSH - o modulo viaja cifrado e so activa se o hash bater.
          mesh-2vm-skills = pkgs.testers.nixosTest {
            name = "nixaios-mesh-skills";
            nodes.a = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.1"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            nodes.b = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.2"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
                "NOSD_JOB_BIND=0.0.0.0:19001"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              start_all()
              a.wait_for_unit("nosd.service")
              b.wait_for_unit("nosd.service")
              a.wait_for_file("/run/nosd/nosd.sock")
              b.wait_for_file("/run/nosd/nosd.sock")
              ca = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              a.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              b.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              a.succeed("cat > /tmp/ident.wat <<'EOF'\n(module (func (export \"run\") (param i32) (result i32) local.get 0))\nEOF")
              a.succeed(ca + " skill-install ident /tmp/ident.wat")
              a.succeed(ca + " skill-run ident --arg 5 | grep -q '\"value\": 5'")
              a.succeed(ca + " skill-push 192.168.50.2:19001 ident | grep -q '\"verified\": true'")
              b.wait_until_succeeds(ca + " skill-list | grep -q 'ident'", timeout=30)
              b.succeed(ca + " skill-run ident --arg 9 | grep -q '\"value\": 9'")
              # S2c: cadeia assinada (assinar -> instalar -> push -> signed ->
              # revogar -> recusar). Usa o CLI, nao inventa assinaturas.
              import json
              r = json.loads(a.succeed(ca + " skill-install sum /tmp/ident.wat --focus sum --version 1.0.0"))
              h = r["result"]["hash"]
              sj = json.loads(a.succeed("sudo " + ca + " skill-sign sum " + h + " --focus sum --version 1.0.0"))
              pub, sig = sj["publisher"], sj["sig"]
              a.succeed(ca + " skill-install sum /tmp/ident.wat --focus sum --version 1.0.0 --publisher " + pub + " --sig " + sig)
              rp = json.loads(a.succeed(ca + " skill-push 192.168.50.2:19001 sum"))
              assert rp["result"]["verified"] is True, rp
              m = json.loads(b.succeed(ca + " crdt-get skill/sum/meta"))
              assert json.loads(m["result"]["value"])["provenance"] == "signed", m
              b.succeed(ca + " skill-revoke " + pub)
              a.fail(ca + " skill-push 192.168.50.2:19001 sum")
              a.fail(ca + " skill-push 192.168.50.2:19001 nao-existe")
            '';
          };

          # M8: anti-entropy de estado (CRDT) pelo link autenticado do M7.
          mesh-2vm-crdt = pkgs.testers.nixosTest {
            name = "nixaios-mesh-crdt";
            nodes.a = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.1"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
                "NOSD_JOB_BIND=0.0.0.0:19001"
                "NOSD_CRDT_BACKEND=nsgdb"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            nodes.b = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.2"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
                "NOSD_JOB_BIND=0.0.0.0:19001"
                "NOSD_CRDT_BACKEND=nsgdb"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              start_all()
              a.wait_for_unit("nosd.service")
              b.wait_for_unit("nosd.service")
              a.wait_for_file("/run/nosd/nosd.sock")
              b.wait_for_file("/run/nosd/nosd.sock")
              ca = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              a.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              b.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              # M8.1d: o CRDT usa o segmento nsgdb (NSGC), nao JSONL.
              a.succeed("[ \"$(head -c 4 /var/lib/nosd/mesh-crdt/seg.bin)\" = NSGC ]")
              b.succeed("[ \"$(head -c 4 /var/lib/nosd/mesh-crdt/seg.bin)\" = NSGC ]")
              a.succeed("test ! -e /var/lib/nosd/crdt.jsonl")
              b.succeed("test ! -e /var/lib/nosd/crdt.jsonl")
              # A escreve e sincroniza -> B ve
              a.succeed(ca + " crdt-put k1 v1")
              s1 = a.succeed(ca + " crdt-sync 192.168.50.2:19001")
              assert '\"verified\": true' in s1, s1
              b.wait_until_succeeds(ca + " crdt-get k1 | grep -q '\"value\": \"v1\"'", timeout=30)
              # B escreve e sincroniza de volta -> A ve, ambos com 2 entradas
              b.succeed(ca + " crdt-put k2 v2")
              b.succeed(ca + " crdt-sync 192.168.50.1:19001")
              a.wait_until_succeeds(ca + " crdt-get k2 | grep -q '\"value\": \"v2\"'", timeout=30)
              assert '\"entries\": 2' in a.succeed(ca + " crdt-get k1"), a.succeed(ca + " crdt-get k1")
              assert '\"entries\": 2' in b.succeed(ca + " crdt-get k1"), b.succeed(ca + " crdt-get k1")
            '';
          };

          # M5.1: 3 nos com falhas reais (latencia, perda total, kill -9 e link
          # down). Prova que a frota sobrevive e converge sem hardware.
          # Nao substitui metal (AWAITING_HW) nem uma frota real com NAT/Wi-Fi.
          mesh-3vm-faults = pkgs.testers.nixosTest {
            name = "nixaios-mesh-faults";
            nodes.a = meshNode3 "192.168.50.1";
            nodes.b = meshNode3 "192.168.50.2";
            nodes.c = meshNode3 "192.168.50.3";
            testScript = ''
              start_all()
              a.wait_for_unit("nosd.service")
              b.wait_for_unit("nosd.service")
              c.wait_for_unit("nosd.service")
              a.wait_for_file("/run/nosd/nosd.sock")
              b.wait_for_file("/run/nosd/nosd.sock")
              c.wait_for_file("/run/nosd/nosd.sock")
              ca = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              # Topologia: cada no fixa os outros dois (TOFU + HB assinado).
              for m in (a, b, c):
                  m.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 2'", timeout=120)
              # Convergencia em cadeia: A -> B -> C.
              a.succeed(ca + " crdt-put k1 v1")
              a.succeed(ca + " crdt-sync 192.168.50.2:19001")
              b.wait_until_succeeds(ca + " crdt-get k1 | grep -q '\"value\": \"v1\"'", timeout=30)
              b.succeed(ca + " crdt-sync 192.168.50.3:19001")
              c.wait_until_succeeds(ca + " crdt-get k1 | grep -q '\"value\": \"v1\"'", timeout=30)

              # netem: 200 ms de latencia nao impedem um job A -> C.
              c.succeed("tc qdisc add dev eth1 root netem delay 200ms")
              a.succeed(ca + " job-submit 192.168.50.3:19001 'oi com latencia' --deadline-ms 10000")
              c.succeed("tc qdisc del dev eth1 root")

              # Kill -9: systemd reinicia (Restart=always) e o log NSGC tem de
              # devolver o estado que C escreveu antes de morrer.
              c.succeed(ca + " crdt-put kc vc")
              c.succeed("systemctl kill -s KILL nosd.service")
              c.wait_until_succeeds(ca + " health", timeout=90)
              c.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 2'", timeout=120)
              c.wait_until_succeeds(ca + " crdt-get kc | grep -q '\"value\": \"vc\"'", timeout=30)
              # O HB do no reiniciado nao pode ser recusado como replay: o seq
              # tem de sobreviver ao restart (senao os peers ficam com metadados
              # velhos para sempre).
              a.fail("journalctl -u nosd -o cat | grep -q 'rejeitado (Replay)'")
              b.fail("journalctl -u nosd -o cat | grep -q 'rejeitado (Replay)'")

              # B escreveu enquanto C reiniciava; C puxa e propaga kc de volta.
              b.succeed(ca + " crdt-put kb vb")
              c.succeed(ca + " crdt-sync 192.168.50.2:19001")
              c.wait_until_succeeds(ca + " crdt-get kb | grep -q '\"value\": \"vb\"'", timeout=30)
              b.succeed(ca + " crdt-sync 192.168.50.1:19001")
              a.wait_until_succeeds(ca + " crdt-get kc | grep -q '\"value\": \"vc\"'", timeout=30)

              # Perda total (netem 100%): A -> C falha, A <-> B continua.
              c.succeed("tc qdisc add dev eth1 root netem loss 100%")
              a.fail(ca + " job-submit 192.168.50.3:19001 oi --deadline-ms 1500")
              a.succeed(ca + " crdt-put kp vp")
              a.succeed(ca + " crdt-sync 192.168.50.2:19001")
              b.wait_until_succeeds(ca + " crdt-get kp | grep -q '\"value\": \"vp\"'", timeout=30)
              c.fail(ca + " crdt-sync 192.168.50.1:19001")
              c.succeed("tc qdisc del dev eth1 root")

              # Particão dura (link down) e recuperação.
              c.succeed("ip link set eth1 down")
              a.fail(ca + " job-submit 192.168.50.3:19001 oi --deadline-ms 1500")
              a.succeed(ca + " crdt-put kq vq")
              a.succeed(ca + " crdt-sync 192.168.50.2:19001")
              b.wait_until_succeeds(ca + " crdt-get kq | grep -q '\"value\": \"vq\"'", timeout=30)
              c.succeed("ip link set eth1 up")
              c.wait_until_succeeds(ca + " crdt-sync 192.168.50.1:19001", timeout=60)
              c.wait_until_succeeds(ca + " crdt-get kq | grep -q '\"value\": \"vq\"'", timeout=30)

              # Convergencia final nos tres, chave a chave (o daemon tambem
              # escreve chaves internas no CRDT: contar entradas seria fragil).
              for m in (a, b, c):
                  for k, v in [("k1", "v1"), ("kc", "vc"), ("kb", "vb"), ("kp", "vp"), ("kq", "vq")]:
                      m.wait_until_succeeds(ca + " crdt-get " + k + " | grep -q '\"value\": \"" + v + "\"'", timeout=90)
            '';
          };

          # M7: jobs remotos com frames assinados sobre TCP. B executa
          # localmente (chat/cpu-echo) e devolve resultado assinado; A verifica.
          mesh-2vm-jobs = pkgs.testers.nixosTest {
            name = "nixaios-mesh-jobs";
            nodes.a = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.1"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            nodes.b = { pkgs, lib, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.2"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_MESH_REQUIRE_SIG=1"
                "NOSD_JOB_BIND=0.0.0.0:19001"
              ];
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              start_all()
              a.wait_for_unit("nosd.service")
              b.wait_for_unit("nosd.service")
              a.wait_for_file("/run/nosd/nosd.sock")
              b.wait_for_file("/run/nosd/nosd.sock")
              ca = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              a.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              b.wait_until_succeeds(ca + " health | grep -q '\"mesh_pinned\": 1'", timeout=90)
              b.wait_until_succeeds("journalctl -u nosd -o cat | grep -q 'job listener bind='", timeout=60)
              r = a.succeed(ca + " job-submit 192.168.50.2:19001 'oi do no A'")
              assert '\"verified\": true' in r, r
              assert '\"ok\": true' in r, r
              a.fail(ca + " job-submit 192.168.50.2:19001 $(head -c 9000 /dev/zero | tr '\\0' x)")
              a.fail(ca + " job-submit 192.168.50.2:19999 oi")
            '';
          };

          # M5 (lab): 2 VMs na mesma VLAN — HB UDP mesh, TOFU pk-bound e
          # eleição stub por capacidade. CRDT wire sync é residual (não testado).
          mesh-2vm = pkgs.testers.nixosTest {
            name = "nixaios-mesh-2vm";
            nodes.a = { pkgs, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.1"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                nodeId = 11;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            nodes.b = { pkgs, ... }: {
              imports = [ self.nixosModules.neural-os ];
              virtualisation.vlans = [ 1 ];
              networking.interfaces.eth1.ipv4.addresses = [
                { address = "192.168.50.2"; prefixLength = 24; }
              ];
              networking.firewall.enable = false;
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 1;
                nodeId = 12;
                meshBind = "0.0.0.0:19000";
                meshBroadcast = "192.168.50.255:19000";
              };
              environment.systemPackages = [ self.packages.${system}.nosd ];
              system.stateVersion = "25.11";
            };
            testScript = ''
              start_all()
              a.wait_for_unit("nosd.service")
              b.wait_for_unit("nosd.service")
              a.wait_for_file("/run/nosd/nosd.sock")
              b.wait_for_file("/run/nosd/nosd.sock")
              ca = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              cb = ca
              # HB UDP: cada nó vê exatamente 1 peer (broadcast na VLAN).
              a.wait_until_succeeds(ca + " health | grep -q '\"mesh_peers\": 1'", timeout=90)
              b.wait_until_succeeds(cb + " health | grep -q '\"mesh_peers\": 1'", timeout=90)
              # TOFU: HB com PK no wire (pk-bound). O settle (7.2s) que
              # destranca ROLE/SkillSync é coberto pelo unit test
              # `nos-mesh::tofu_settles_after_window` (o JSON `mesh` é uma
              # string escapada, má para grep).
              a.wait_until_succeeds("journalctl -u nosd -o cat | grep -q 'pk=true'", timeout=90)
              # Eleição stub: capacidade igual -> menor node id (11) é Master.
              a.wait_until_succeeds(ca + " health | grep -q 'Master'", timeout=90)
              b.wait_until_succeeds(cb + " health | grep -q 'Worker'", timeout=90)
              print(a.succeed(ca + " health"))
            '';
          };
        });

      nixosConfigurations.nixaios-iso-hw = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [
          ({ pkgs, ... }: {
            nixpkgs.config.allowUnfree = true;
            nixpkgs.overlays = [
              (final: prev: {
                nosd = self.packages.x86_64-linux.nosd;
              })
            ];
          })
          self.nixosModules.default
          ./images/iso.nix
          ./images/hw.nix
          ({ pkgs, self, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
            # Brain embutido (symlink content-addressed no models): o iso-hw
            # arranca com engine=llama sem rede nem seed manual (ADR-0118
            # addenda 0118-qwen-embedded-brain). NUNCA na ISO de release.
            services.nosdBrain = {
              enable = true;
              package = self.packages.${pkgs.system}.qwen35-08b-q4km;
            };
          })
        ];
      };

      # Variante CUDA do iso-hw: mesmo stack + NVIDIA proprietário e
      # llama-cpp-cuda (RTX 3050). Ver images/hw-cuda.nix.
      nixosConfigurations.nixaios-iso-hw-cuda = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [
          ({ pkgs, ... }: {
            nixpkgs.config.allowUnfree = true;
            nixpkgs.overlays = [
              (final: prev: {
                nosd = self.packages.x86_64-linux.nosd;
              })
            ];
          })
          self.nixosModules.default
          ./images/iso.nix
          ./images/hw.nix
          ./images/hw-cuda.nix
          ({ pkgs, self, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
            services.nosdBrain = {
              enable = true;
              package = self.packages.${pkgs.system}.qwen35-08b-q4km;
            };
          })
        ];
      };

      # Opcional: hw-cuda + nvidia legacy para Pascal (1050).
      nixosConfigurations.nixaios-iso-hw-cuda-legacy = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [
          ({ pkgs, ... }: {
            nixpkgs.config.allowUnfree = true;
            nixpkgs.overlays = [
              (final: prev: {
                nosd = self.packages.x86_64-linux.nosd;
              })
            ];
          })
          self.nixosModules.default
          ./images/iso.nix
          ./images/hw.nix
          ./images/hw-cuda.nix
          ./images/hw-cuda-legacy.nix
          ({ pkgs, self, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
            services.nosdBrain = {
              enable = true;
              package = self.packages.${pkgs.system}.qwen35-08b-q4km;
            };
          })
        ];
      };

      # HW CUDA single-model: só Qwen3.5-4B-Q6_K na dGPU (sem 0.8B/S2).
      nixosConfigurations.nixaios-iso-hw-cuda-4b = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [
          ({ pkgs, ... }: {
            nixpkgs.config.allowUnfree = true;
            nixpkgs.overlays = [
              (final: prev: {
                nosd = self.packages.x86_64-linux.nosd;
              })
            ];
          })
          self.nixosModules.default
          ./images/iso.nix
          ./images/hw-4b.nix
          ./images/hw-cuda-4b.nix
          ({ pkgs, self, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
            services.nosdBrain = {
              enable = true;
              package = self.packages.${pkgs.system}.qwen35-4b-q6k;
              name = "active.gguf";
            };
          })
        ];
      };

      nixosConfigurations.nixaios-iso-demo = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [
          ({ pkgs, ... }: {
            nixpkgs.config.allowUnfree = true;
            nixpkgs.overlays = [
              (final: prev: {
                nosd = self.packages.x86_64-linux.nosd;
              })
            ];
          })
          self.nixosModules.default
          ./images/iso.nix
          ./images/demo.nix
          ({ pkgs, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
          })
        ];
      };

      nixosConfigurations.nixaios-iso = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self; };
        modules = [
          ({ pkgs, ... }: {
            nixpkgs.config.allowUnfree = true;
            nixpkgs.overlays = [
              (final: prev: {
                nosd = self.packages.x86_64-linux.nosd;
              })
            ];
          })
          self.nixosModules.default
          ./images/iso.nix
          ({ pkgs, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
          })
        ];
      };

      devShells = forAllSystems (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; config.allowUnfree = true; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              (rust-bin.stable.latest.default.override { extensions = [ "rust-src" "clippy" "rustfmt" ]; })
              pkg-config
              openssl
              cmake
              clang
              fontconfig
              freetype
              libxkbcommon
              wayland
            ];
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            shellHook = ''
              export NOSD_ENGINE_DIR="$PWD/../result/lib/nosd/engines"
              echo "NixAIOS devShell — cargo test; Slint: --features nos-jarbas/slint-ui"
            '';
          };
        });
    };
}
