# Sessão 2026-11-05 — VISÃO no topo da direita, piper a carregar, Chromium sem Gemini

Segunda metade da sessão: fechar derrotas que se podiam converter em vitória **sem
metal**. Três defeitos, três causas, todas provadas no host.

## 1. HUD — VISÃO deixa o fundo da coluna esquerda

`b346618`: o painel `04 VISÃO` era o **último** filho da `railLeft` (coluna a
25vw, com `overflow-y:auto`) e no kiosk 720p o `@media (max-height:800px)`
baixava-lhe o `min-height` para **96px**. Passou para o **topo da `railRight`**,
antes de `05 ATIVIDADES`.

- `#visionPanel{flex:0 0 auto;min-height:170px}` — vence o `.panel.grow`
  (`flex:1`) para não roubar altura às atividades.
- A 720p o feed passou a **empilhado**: `.vision-body{flex-direction:column}`,
  feed a `width:100%` e `aspect-ratio:16/10`, meta numa faixa horizontal
  (métricas escondidas). Medido no artefacto: feed **168×126 → 297×185**
  (2,6× a área), painel 167 → 255 px.
- Artefacto: [`docs/evidence/artifacts/orb_hud_vision_right_720p.png`](../evidence/artifacts/orb_hud_vision_right_720p.png)
  — render real do HTML a 1280×720 (portão de sessão escondido; sem daemon, os
  campos ficam `—`). Prova de **layout**, não de cage/DRM.

Nota de método: a `#app` do HUD tem `opacity:0` + transição até haver link. Para
tirar a captura foi preciso `transition:none!important` — `!important` sozinho
**não** vence uma transição a correr (as transições ficam acima de `!important`
na cascata). Sem isso a imagem sai preta e parece bug do produto.

## 2. Voz — o piper nunca carregou (D4/A6)

`A6` dizia `io: No such file or directory (os error 2)` com `tts_ready: true` e
os ficheiros no sítio. O erro é de `Command::new(piper).spawn()?` → `VoiceError::Io`:
é o **execve** a falhar, não um ficheiro em falta.

```
readelf -l $out/piper → [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]
```

O binário é o tarball rhasspy/piper (ELF de distro) e a derivação tinha
`dontFixup = true`: no NixOS **não existe** `/lib64/ld-linux-x86-64.so.2`, logo
`execve` devolve ENOENT para um ficheiro que existe e passa `-x`. Mesmo género
da lição 59 (`nvidia-smi`): o ficheiro e as flags podem estar certos e o binário
não carregar.

Fix (`flake.nix`, `piper-standalone`):

1. `patchelf --set-interpreter ${pkgs.glibc.out}/lib/ld-linux-x86-64.so.2` nos
   dois ELF (`piper`, `piper_phonemize`).
2. Wrapper `$out/piper` com `LD_LIBRARY_PATH=$out:<glibc>/lib:<gcc cc.lib>/lib`
   — `DT_RUNPATH` não se propaga às dependências indirectas, e `libstdc++.so.6`
   (para o `libonnxruntime` do bundle) só existe no store.

Prova no host: `--help` **rc=0**; síntese real com a voz pinada
(`piper-pt-br-faber-medium-{onnx,json}`) → `out.wav` **170 260 B**, RIFF PCM
16-bit mono 22 050 Hz, RTF **0,0337** (3,46 s de áudio em 0,117 s).
Not-tested: saída de áudio no metal (`play_ok`, `aplay`/PipeWire) — o
`card 1 … Subdevices: 0/1` do A1 diz **cartão ocupado**, não ausente.

## 3. Chromium — nem "Gemini" nem Vulkan

O `README` do bundle chamava *"adaptador GPU não detectado"* às linhas:

```
services/on_device_model/ml/gpu_blocklist.cc:120  Unable to get gpu adapter
```

Isso é o **serviço de modelo on-device** (o "Gemini Nano" do Chromium) a sondar
GPU para features de IA que não usamos — não é o compositor. As linhas que
importam são as do render:

```
media/gpu/vaapi/vaapi_wrapper.cc  vaInitialize failed: unknown libva error
ui/ozone/platform/wayland/gpu/wayland_surface_factory.cc:249
  '--ozone-platform=wayland' is not compatible with Vulkan
```

Fix em `modules/jarbas-cage.nix`:

- `--disable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan,OptimizationGuideModelDownloading,OptimizationHints,OptimizationHintsFetching,OptimizationTargetPrediction,OnDeviceModel,OptimizationGuideOnDeviceModel`
- `--disable-background-networking --disable-component-update --disable-sync
  --disable-domain-reliability --no-pings --metrics-recording-only`
  (o log tinha `gcm/registration_request.cc PHONE_REGISTRATION_ERROR`: o kiosk
  estava a telefonar para casa).

Bootstrap/cookie do HUD continuam em loopback, portanto não há perda funcional.
Not-tested: GPU a desenhar no cage — só um ecrã o prova.

## Verificação desta metade

- `nix-instantiate --parse flake.nix` + `modules/jarbas-cage.nix`: OK.
- `nix build .#piper-standalone` + `--help` rc=0 + síntese WAV (acima).
- `nix build .#iso-hw-cuda` rc=0 (ISO reconstruída com as três correcções).
- Screenshot 720p do HUD, medido por `getBoundingClientRect` e inspeccionado a
  olho (`orb_hud_vision_right_720p.png`).

**Not-tested:** nenhum boot metal nesta metade; áudio audível, GPU no cage e
`nvidia-smi --query-gpu` continuam AWAITING_HW.
