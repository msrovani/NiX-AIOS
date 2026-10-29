# modules/nosd-brain.nix — brain default do iso-hw
# (ADR-0118 addenda `0118-qwen-embedded-brain.md`)
#
# O GGUF pinado via `self.packages.${system}.qwen35-08b-q4km` entra em
# `models` como SYMLINK de store path (content-addressed): nada é baixado no
# primeiro boot e nada é copiado — a ISO embute **a derivação referenciada**.
# NUNCA usar na ISO de release (S1.2); metal/iso-hw MAIS a proposta de
# upgrade pós-boot (card HITL blob.fetch) é o canário.
#
# Uso no config do iso-hw:
#   services.nosdBrain = {
#     enable = true;
#     package = self.packages.${system}.qwen35-08b-q4km;
#   };
{ config, lib, ... }:
let
  cfg = config.services.nosdBrain;
in
{
  options.services.nosdBrain = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Brain default embutido (symlink de store path no models do nosd)";
    };
    package = lib.mkOption {
      type = lib.types.package;
      description = "GGUF content-addressed (ex.: qwen35-08b-q4km)";
    };
    name = lib.mkOption {
      type = lib.types.str;
      default = "qwen3.5-0.8b-q4km.gguf";
      description = "Nome do ficheiro em /var/lib/nosd/models (visível a model.list/hotswap)";
    };
  };

  config = lib.mkIf cfg.enable {
    # L+ : cabeça do symlink a cada boot — idempotente e anti-drift
    # (se alguém apagar/repor manualmente o models, volta a apontar ao store).
    # Ordem: o `d /var/lib/nosd/models` do neural-os corre antes (o módulo
    # é importado depois dele no nixosModules.default).
    systemd.tmpfiles.rules = [
      "L+ /var/lib/nosd/models/${cfg.name} - - - - ${cfg.package}"
    ];
  };
}
