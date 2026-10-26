# Triagem — prompt “ISO Minimal AIOS / setup-wizard / Orb DRM”

**Data:** 2026-10-25 · **Escopo:** não regenerar árvore paralela; mapear → `nixaios/` existente (ADR-0118 / 0113).

## Veredito

| Pedido do prompt | Decisão | Onde / nota |
|------------------|---------|-------------|
| ISO minimal sem GNOME/KDE/XFCE | **JÁ FEITO** | `images/iso.nix` + `transmute-stage` + `multi-user.target` |
| Wizard TUI rede/teclado/idioma/fuso | **JÁ FEITO** | `nos-firstboot` (não `setup-wizard.sh` novo) |
| Persistência respostas → ficheiro Nix | **ADOPT** | `host-configuration.nix` em `/var/lib/nosd` + FAT; **sem** `nixos-rebuild switch` na live |
| `nix-channel --update` + rebuild no 1º boot | **REJECT** | Live ISO imutável; anel Cortex = staging + HITL (`cortex-nix-ring`) |
| Clonar GitHub + compilar crates no boot | **REJECT** | `nosd` já vem no flake da ISO; compile-on-boot = offline-hostile + horas |
| Pipeline cage / Wayland leve | **JÁ FEITO** | `jarbas-cage.nix` + seatd-launch + Slint |
| DRM/KMS DoubleBuffer + Orb 3 Z-order nativo | **DEFER** | Path bare-metal `crates/jarbas`; host = **ADAPT** Slint HUD (addendum cage). Port pleno = onda dedicada |
| hugepages 1G×4 default | **REJECT default** | Brick em ≤8 GiB RAM; opt-in `aiosHostPerf.hugepages1g` |
| `vm.swappiness=0`, governor performance | **ADOPT** | `modules/aios-host-perf.nix` |
| `oom_score_adj=-1000` no nosd | **ADOPT** | `neural-os.nix` `OOMScoreAdjust` |
| rtprio=99 / memlock unlimited (nosd) | **ADOPT suave** | `LimitRTPRIO` / `LimitMEMLOCK` no unit (não PAM global 99) |
| eBPF / io_uring /kvm irrestrito | **DEFER** | Não necessário ao gate remig; kvm opt-in depois |
| Flake/config/modules novos do zero | **REJECT** | Duplicaria `flake.nix` + modules já wired |

## Mapa prompt → ficheiros reais

| Prompt inventava | Canónico NixAIOS |
|------------------|------------------|
| `setup-wizard.sh` | `modules/transmute-stage.nix` → `nos-firstboot` |
| `configuration.nix` monolito | `images/iso.nix` + modules flake |
| `modules/nosd-service.nix` | `modules/neural-os.nix` |
| `modules/gui-jarbas.nix` | `modules/jarbas-cage.nix` |
| Orb compositor 3 camadas | residual; UI host = `nos-jarbas --ui` |

## Honesty (ADR-0113)

- GUI metal estável = PARTIAL (cage+seatd); Orb FFT 16 bins no host = UNKNOWN / not wired.
- “Compilar neural-os-core no firstboot” ≠ propriedade demonstrável nesta ISO.
