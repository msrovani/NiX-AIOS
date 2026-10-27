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
            version = "0.6.0-alpha.2";
            src = ./packages/nosd;
            cargoLock.lockFile = ./packages/nosd/Cargo.lock;
            # Onda 5: JARBAS Slint no binário ISO (cage --ui).
            cargoExtraArgs = "--workspace --features nos-jarbas/slint-ui";
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
        in
        {
          nixaios-vm = pkgs.testers.nixosTest {
            name = "nixaios-vm";
            nodes.machine = { ... }: {
              imports = [ self.nixosModules.neural-os ];
              services.nosd = {
                enable = true;
                package = self.packages.${system}.nosd;
                hermesTickSecs = 0;
              };
              environment.systemPackages = [ self.packages.${system}.nosd ];
              # 0.6: tiny GGUF em /var/lib/nosd/models; nosd faz spawn de llama-server.
              system.activationScripts.nixaiosTinyModel = ''
                mkdir -p /var/lib/nosd/models
                cp ${tinyGguf} /var/lib/nosd/models/stories15M-q4_0.gguf
                chown -R nosd:nosd /var/lib/nosd/models
                chmod 0755 /var/lib/nosd/models
                chmod 0644 /var/lib/nosd/models/stories15M-q4_0.gguf
              '';
              system.stateVersion = "25.11";
            };
            testScript = ''
              machine.wait_for_unit("nosd.service")
              machine.wait_for_file("/run/nosd/nosd.sock")
              nosctl = "${self.packages.${system}.nosd}/bin/nosctl --socket /run/nosd/nosd.sock"
              machine.succeed(nosctl + " health")
              machine.succeed(nosctl + " health | grep -q 'llama-server-http'")
              machine.succeed(nosctl + " chat 'Once upon a time' | grep -q '\"text\"'")
              machine.succeed(nosctl + " unload")
              machine.succeed(nosctl + " health | grep -q 'cpu-echo'")
              machine.succeed(nosctl + " load /var/lib/nosd/models/stories15M-q4_0.gguf")
              machine.succeed(nosctl + " health | grep -q 'llama-server-http'")
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
