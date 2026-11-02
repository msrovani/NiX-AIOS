# ADR-0128 (filho) — M3b metal: áudio, journal e critério OBSERVED→VERIFIED

**Status:** ACEITE (doc NEXUS-0010 + script FORGE `scripts/metal-m3b-harvest.sh`)
**Pai:** ADR-0128 *focus M3 Boot→JARBAS* (`accepted_as_focus`, FORGE-0007)
**Âmbito:** `Ring3_Linux/metal` · documento + checklist + script de colheita.
Evidência metal continua AWAITING_HW até bundle numa boot Core7.
**Referências:** `AGENTS.md` §7 (BitNet/Ring0/X11 fora), ADR-0113 (honesty),
`docs/METAL_CAMPAIGN.md`, `docs/HW_TEST.md`, G1/G5 de FORGE-0006.

> Regra que este ADR existe para impedir: **cage ACTIVE ≠ voz**. Um HUD a subir
> não prova áudio, e `aplay -l` a listar um card não prova que o PipeWire do
> kiosk tem grafo. Cada estágio tem de ter a sua própria linha de evidência.

---

## 1. Contexto real (paths verificados, não presumidos)

| Peça | Onde está de facto | Nota |
|------|--------------------|------|
| kiosk | `modules/jarbas-cage.nix` (`services.jarbasCage`) | não existe `modules/cage/` |
| primeira boot | `modules/transmute/firstboot.sh` + `modules/transmute-stage.nix` | não existe `modules/nosd/firstboot.nix` |
| gatilho do GUI | `/var/lib/nosd/gui-start` tocado em `modules/transmute/hotswap-aios.sh:62` e `transmute-stage.nix:294` | `jarbas-cage` tem `ConditionPathExists` |
| runtime dir | `/run/nosd/user` (não `/run/user/...`) | logind monta tmpfs e esconde o que foi criado antes |
| probe ALSA | `jarbas-cage.nix:245` → `aplay -l` para `$XDG_RUNTIME_DIR/alsa-cards.txt` | já existe; ninguém o lê no aceite |
| PipeWire | lançado **dentro** do wrapper cinematic (`pipewire`, `wireplumber`, `pipewire-pulse`) | por isso `wpctl` só funciona com o `XDG_RUNTIME_DIR` do kiosk |
| DeviceAllow | `jarbas-cage` e unit do motor: `char-drm`, `char-input`, `char-tty`, `char-video4linux`, `char-alsa` | `DevicePolicy=closed` implícita |
| journal export | `modules/log-fat32.nix:227-229` → `boots/<STAMP>_<HOST>` + `LATEST` a apontar para lá | FAT `NIXAIOSLOG` |

Unidades que contam para o aceite (nomes exactos do tree):
`nosd` · `nosd-early-score` · `nosd-hotswap-aios` · `nosd-boot-persist-check` ·
`jarbas-cage` · `jarbas-hub` · `jarbas-show-vt` · `nixaios-log-mount` ·
`nixaios-log-export` · (quando `services.nosdEngine.supervise=true`)
`nosd-engine-s2`.

---

## 2. Decisão

M3b em metal só passa a `VERIFIED` através de um **protocolo de 6 estágios de
áudio** e de um **evidence bundle** colhido na mesma boot que os produziu.
Nenhum estágio pode ser inferido a partir do seguinte.

### 2.1 Estágios de áudio (cada um com prova própria)

| # | Estágio | Prova mínima | Falha ⇒ |
|---|---------|--------------|---------|
| A1 | card ALSA presente | `aplay -l` com `card 0` e `device 0` | `audio_absent` (honesto, não OK) |
| A2 | grafo PipeWire do kiosk | `wpctl status` mostra sink **e** source, `pipewire-0` socket em `/run/nosd/user` | `pipewire_absent` |
| A3 | saída audível | `espeak-ng` greeting (já agendado no wrapper) + `pw-cli info` com o nó activo | `sink_silent` |
| A4 | captura | `arecord -d 2 -f S16_LE /tmp/x.wav` + bytes > 44 | `mic_absent` |
| A5 | STT | `voice.status` = `Ready` **ou** `EngineAbsent` com motivo | `stt_absent` |
| A6 | TTS→out | `voice.turn` produz output e o sink mostra actividade | `tts_silent` |

Regra ADR-0113: A1–A4 ausentes = **`absent`**, nunca "0%" ou "OK por omissão".
`mic OFF` no HUD é resultado válido desde que rotulado `absent` com a prova de A1.

### 2.2 Checklist de colheita (uma sessão, na máquina Core7)

```bash
# 0. estado das units (sem sudo para ler journal do grupo)
systemctl --no-pager --state=failed list-units 'nosd*' 'jarbas*' 'nixaios*'
systemctl show jarbas-cage -p DeviceAllow -p SupplementaryGroups -p ConditionResult

# 1. áudio: A1 (card) e A2 (grafo, no runtime dir do kiosk)
aplay -l
runuser -u nosd -- env XDG_RUNTIME_DIR=/run/nosd/user wpctl status
runuser -u nosd -- env XDG_RUNTIME_DIR=/run/nosd/user pw-cli info 0

# 2. A3/A4
runuser -u nosd -- env XDG_RUNTIME_DIR=/run/nosd/user arecord -d 2 -f S16_LE /tmp/mic.wav
ls -l /tmp/mic.wav   # > 44 bytes

# 3. vosk/piper pelo daemon (A5/A6)
nosctl voice-status
nosctl voice-turn "teste de audio"   # se Ready

# 4. journal (o que o stick deve conter)
journalctl -b -u nosd -u jarbas-cage -u jarbas-hub -u nosd-hotswap-aios \
  -u nosd-engine-s2 --no-pager > journal.txt
journalctl -b -u nixaios-log-mount -u nixaios-log-export --no-pager > journal-log.txt

# 5. stick FAT (Offline no Windows é o blocker G1 — Online em diskmgmt ou wsl --mount)
ls /run/media/*/            # ponto de montagem do export
cat <mnt>/LATEST            # ex.: boots/20261102T041500Z_nixaios-node
ls -la "<mnt>/$(cat <mnt>/LATEST)"
```

### 2.3 Evidence bundle (obrigatório para `VERIFIED`)

`docs/evidence/hw/<YYYY-MM-DD_HHMMZ>/` com:

- `README.md` — host, GPU, kernel, o que foi observado e o que **não** foi;
- `journal.txt`, `journal-log.txt`, `alsa.txt` (`aplay -l`), `wpctl.txt`,
  `pw-cli.txt`, `mic.wav.sha256`, `voice-status.json`;
- `stick.txt` — `LATEST` + listagem do directório apontado;
- `SHA256SUMS` de tudo o acima;
- linha `Tested:` / `Not-tested:` (AGENTS.md §36).

### 2.4 Elevação de claim

| De | Para | Exige |
|----|------|-------|
| — | `IMPLEMENTED` | código + teste de contrato no host |
| `IMPLEMENTED` | `OBSERVED` | **1** boot em metal/QEMU com artefacto |
| `OBSERVED` | `VERIFIED` | **3** boots consecutivos com A1–A6 verdes, bundle commitado, `gate-boot-to-jarbas.sh` sem `SKIP` nos estágios obrigatórios |
| qualquer | `AWAITING_HW` | só dependência física (GPU/mic ausente), **nunca** bug de software |

`QEMU` nunca eleva `metal`: `Ring3_Linux/qemu OBSERVED ≠ Ring3_Linux/metal VERIFIED`.

---

## 3. Fail-closed

1. `wpctl`/`pw-cli` sem socket em `XDG_RUNTIME_DIR` ⇒ `pipewire_absent`, não
   "áudio OK porque o grupo `audio` existe".
2. `aplay -l` com card mas sem sink no grafo ⇒ `sink_absent` (o `char-alsa` no
   `DeviceAllow` abre o device, não prova mistura).
3. `boots/LATEST` ausente ou a apontar para directório vazio ⇒ bundle
   **inválido** para `VERIFIED` (o jornal não foi exportado).
4. `peers=0` **não** é aceite deste ADR — é `0118-mesh-unicast-seeds`
   (ANVIL-0002 / ack FORGE-0008). Não duplicar o fix aqui.
5. Se o checklist revelar bug de código, abre-se patch separado **fora** deste
   ADR, com `Tested:`/`Not-tested:` no corpo do commit.

## 4. Non-goals

- Armar ring (`armRing=false` continua), BitNet nativo, X11, wasmtime path A.
- Mexer em `nosd`/`jarbas-cage` neste ADR (só checklist + evidência).
- Declarar `M4 VERIFIED` por QEMU ou por um único boot.

## 5. Pedido

`FORGE`: aceitar como ADR filho de 0128 (Q1+Q3 de FORGE-0006/0007) e usar
§2.2 como guião da próxima sessão no Core7. `ANVIL`: a lane de verificação
continua tua; este ADR não pede builds — pede **um** boot com colheita.
`HUMAN`: autoridade para decidir se a colheita acontece antes ou depois da
tranche `nix.propose-ORB`.
