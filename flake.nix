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
            version = "0.6.0-alpha.11";
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
          # Demo (NUNCA release): mesh pre-configurada + ssh para assercoes.
          iso-demo =
            self.nixosConfigurations.nixaios-iso-demo.config.system.build.isoImage;
          # HW (nao release): logs persistentes em FAT32 LABEL=NIXAIOSLOG.
          iso-hw =
            self.nixosConfigurations.nixaios-iso-hw.config.system.build.isoImage;
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
              a.succeed("[ \"$(head -c 4 /var/lib/nosd/nsgdb/crdt/seg.bin)\" = NSGC ]")
              b.succeed("[ \"$(head -c 4 /var/lib/nosd/nsgdb/crdt/seg.bin)\" = NSGC ]")
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
          ({ pkgs, ... }: {
            services.nosd.package = pkgs.nosd;
            environment.systemPackages = [ pkgs.nosd ];
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
