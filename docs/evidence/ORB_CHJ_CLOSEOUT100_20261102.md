# ORB / CHJ / fábrica — closeout 100% residual (2026-11-02)

## Scope

Fecho do residual: prefs data-dir, voice picker, hibernate/suspend HITL,
CorrectPlan→OPAR, improve.check probes locais, net.monitor host inject,
limpeza e docs.

## Code complete

- `prefs.get` / `prefs.set` → `<data-dir>/ui_prefs.json` (allowlist)
- `voice.status` + `engines`/`models`/`tts_engine` + ORB selects
- `system.hibernate` + `hibernate_capability` (`NOSD_HIBERNATE=1`)
- DIAG `PROPOR SUSPEND` → `improve.propose` kind suspend
- deny HITL → `correct_plan` + `record_opar(improve)`
- `improve.check` com kernel candidate ficheiro/env + flake.lock path
- `skill.run` inject `observe.net` para net.monitor; smoke no accept
- Intent PT coloquial “app de monitoramento da rede”

## Tested

- `Tested: none (code-complete only)` — fecho sem gate de metal/ISO nesta tranche.
  Recompilar host se disponível: `cargo test -p nosd --lib prefs kiosk`.

## Not-tested

- Metal DPMS / suspend / hibernate
- Piper greeting real + voice picker no cage
- ISO rebuild
- Mesh seek hit
- skill e2e no kiosk cinematográfico
- `NOSD_IMPROVE_CHECK=1` com ficheiros pinados em metal

## Honesty

- Ring Nix desarmado.
- Sem phone-home nos probes.
- Hibernate ausente se env/sysfs falha — sem card falsa.
- net.monitor = WASM stub + host inject (não sniffer).
