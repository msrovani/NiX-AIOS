# modules/nosd-tts.nix — attach opcional ao TTS de alta qualidade
# (audio.cpp / Audio8 TTS GGUF). Ver docs/TTS.md.
#
# OFF por omissão: não entra na ISO; o motor corre onde houver GPU (host/frota)
# e o nosd só o invoca por caminho absoluto. Sem Python no appliance.
{ config, lib, ... }:
let
  cfg = config.services.nosdTts;
in
{
  options.services.nosdTts = {
    enable = lib.mkEnableOption "TTS de alta qualidade (audio.cpp / Audio8 GGUF)";
    package = lib.mkOption {
      type = lib.types.package;
      description = "Pacote que fornece bin/audiocpp_cli";
    };
    model = lib.mkOption {
      type = lib.types.path;
      description = "GGUF audio8-tts-realistic-0.6b-{q8_0,bf16}.gguf";
    };
    backend = lib.mkOption {
      type = lib.types.enum [ "cpu" "cuda" "vulkan" ];
      default = "cpu";
      description = "Backend do audio.cpp (sem CUDA validada, cpu/vulkan)";
    };
  };

  config = lib.mkIf cfg.enable {
    # O nos-voice lê estas variáveis; binário absoluto (o PATH não decide).
    systemd.services.nosd.serviceConfig.Environment = lib.mkAfter [
      "NOSD_TTS_BIN=${cfg.package}/bin/audiocpp_cli"
      "NOSD_TTS_MODEL=${cfg.model}"
      "NOSD_TTS_BACKEND=${cfg.backend}"
    ];
  };
}
