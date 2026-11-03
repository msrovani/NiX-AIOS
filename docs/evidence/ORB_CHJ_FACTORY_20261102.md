# ORB + CHJ melhorias + fábrica skill — evidência 2026-11-02

## Scope

Fase 1 controlos ORB (net/audio/mic/idle/TTS), Fase 2 `ImprovePropose`,
Fase 3 factory templates + chat intent → HITL → `skill.install`.

## Tested (host)

- Código in-tree: `observe.net` / `observe.audio`, kiosk allowlist,
  `ImprovePropose`, `factory.plan` templates (`net.monitor`), chat router,
  `ui.action` install on `skill.install`, `display.blank`, cage Piper→espeak.
- Unit tests intended:
  `cargo test -p nos-observe --lib collect_net_view`
  `cargo test -p nos-hermes improve`
  `cargo test -p nos-factory`
  `cargo test -p nosd --lib kiosk_methods`

## Not-tested

- Metal DPMS / `systemctl suspend` / hibernate
- Piper greeting real no cage (timeout path)
- QEMU / ISO rebuild desta tranche
- Mesh seek hit path for factory
- Codegen LLM→wasm (explicitamente NOT_DONE)

## Honesty

- Ring Nix continua desarmado.
- `net.monitor` template = WASM stub JSON + cap `log`; telemetria real via
  `observe.net` no host, não socket no wasm.
- Suspend nunca automático — só grant `system.suspend`.
