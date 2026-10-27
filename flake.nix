{
  description = "NixAIOS — nosd daemon + JARBAS kiosk on NixOS (ADR-0118)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    nixos-generators.url = "github:nix-community/nixos-generators";
    nixos-generators.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, nixos-generators, ... }:
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
            version = "0.6.0-alpha.8";
            # Sem filtro, `cargo test` no host deixa packages/nosd/target/
            # (GBs) e o Nix copia-o read-only para a store; o build seguinte
            # rebenta com "cp: ... target/...: Read-only file system".
            src = builtins.path {
              path = ./packages/nosd;
              name = "nosd-src";
              filter = path: _type:
                let n = builtins.baseNameOf path; in
                n != "target" && n != ".direnv" && n != "result";
            };
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
        } // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") {
          # Convenience: nix build .#iso (from WSL/Linux)
          iso = self.nixosConfigurations.nixaios-iso.config.system.build.isoImage;
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
      nixosModules.default = { ... }: {
        imports = [
          self.nixosModules.neural-os
          self.nixosModules.jarbas-cage
          self.nixosModules.ai-managed
          self.nixosModules.log-fat32
          self.nixosModules.cortex-nix-ring
          self.nixosModules.nosd-engine
          self.nixosModules.nosd-early
          self.nixosModules.transmute-stage
          self.nixosModules.aios-host-perf
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
        in
        {
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
              systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
                "NOSD_LOCALE=en_US.UTF-8"
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
