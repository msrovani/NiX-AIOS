# Sessão 2026-11-05 — SIGSEGV do `nvidia-smi`: causa raiz provada no host

Investigação do defeito D2 do bundle [`docs/evidence/hw/2026-11-03_2346Z`](../evidence/hw/2026-11-03_2346Z/)
sem metal e sem coredump: só com a linha do `dmesg` e o binário do próprio flake.

## O sintoma (o que existe, e o que não existe)

- `dmesg-boot.log:1065`:
  `nvidia-smi[1345]: segfault at 401488 ip 00007754bdf17a74 sp … error 7 in ld-linux-x86-64.so.2[25a74,7754bdef3000+2e000]`
- `journal-boot.log:87` — `systemd-coredump[1347]: Process 1345 (nvidia-smi) of user 0 dumped core.`
- `hw-info.log:66` — `1345 Falha de segmentação (core dumped) /run/current-system/sw/bin/nvidia-smi --query-gpu=…`
- **Não há ficheiro de core no bundle**: o `systemd-coredump` guarda em
  `/var/lib/systemd/coredump`, fora da exportação. Não se "abre o coredump";
  analisa-se o fault. Mesmo endereço (`0x401488`) nos três boots
  (31/10 16:44Z ×2, 3/11 23:46Z) e com users 998 e 0 → defeito estrutural.

## Leitura do fault

| Campo | Significado |
|---|---|
| `error 7` | `present \| write \| user` — **escrita** a página presente e de utilizador, sem permissão de escrita (violação de protecção, não page-fault de endereço inválido) |
| `ip` em `ld-linux` | a falha é no **link dinâmico**, antes de `main` |
| `at 401488` | endereço baixo, na imagem do próprio `nvidia-smi` (non-PIE, base 0x400000) |

Geometria do binário (`nvidia-x11-595.104.02-bin`, lida com o parser do repo):

```
LOAD va=0x4002c4 memsz=0x2fbc flags=0x4 (R)   ← .dynamic vive aqui
PT_DYNAMIC va=0x4013a0 memsz=0x1f0
0x401488 = 0x4013a0 + 0xe8 → entrada 14, 2.º campo = DT_DEBUG.d_ptr
GNU_RELRO va=0x3ff318 memsz=0x53             ← não cobre o fault (RELRO não é a causa)
```

O `ld.so` escreve `r_debug` no slot `DT_DEBUG` do executável principal ao
arrancar. `patchelf` (nixpkgs) tinha posto esse `PT_LOAD` só `PF_R` → SIGSEGV.

## Reprodução no host, com o glibc da própria ISO

`glibc-2.44-57` do flake (`...(pkgs).glibc.out/lib/ld-linux-x86-64.so.2`), com
`--library-path <glibc>/lib:<nvidia-bin>/lib`:

| binário | resultado |
|---|---|
| original (antes do fix) | **rc=139 SIGSEGV** |
| `scripts/fix-nvidia-elf-pf-w.py` (OR `PF_W` em todos os `PT_LOAD`) | **rc=0** + `NVIDIA System Management Interface -- v595.104.02` |
| `--query-gpu=name,memory.total` | rc=9 — *"couldn't communicate with the NVIDIA driver"* (carregou e correu; sem hardware é o esperado) |
| variante sem `PT_GNU_RELRO` | rc=0 (confirma que RELRO não era o problema) |

Nota operacional: no nixpkgs o output **default** do `glibc` é `bin`, que
**não** contém `ld-linux`; o loader está em `${glibc.out}/lib/`.

## Porque o metal crashou com o fix no store

O fix estava certo. O que correu foi outro binário:

1. `hardware.video.nvidia` faz `environment.systemPackages = [ nvidia_x11.bin ]`
   (`nixos/modules/hardware/video/nvidia.nix:512`).
2. nixpkgs dá `meta.priority = 4` ao `nvidia_x11` (`pkgs/os-specific/linux/nvidia-x11/generic.nix:334`,
   posto para a colisão de `libglx.so` com o xorg).
3. o `system-path` usa `buildEnv { ignoreCollisions = true }` (`config/system-path.nix:211`)
   e **prioridade menor ganha**.
4. `nvidia-bin-elf-pf-w` tinha o default **5** → perdia o symlink
   `bin/nvidia-smi`. O `/run/current-system/sw/bin/nvidia-smi` era o ELF por
   corrigir — precisamente o caminho que o `hw-info.log` executou.

O `NOSD_NVIDIA_SMI` (env do nosd) apontava ao corrigido; por isso o HUD exibia
o path `…-nvidia-bin-elf-pf-w/…` enquanto o ficheiro executado era outro.

## Correções

- `lib.hiPrio nvidiaBinFixed` em `images/hw-cuda.nix` e `images/hw-cuda-4b.nix`
  (priority < 4 → ganha a colisão do perfil).
- Check novo `checks.x86_64-linux.nvidia-smi-loads`: corre o ELF sob o loader
  do alvo (`--help`, exige rc=0 **e** o banner) e falha se a prioridade do fix
  não ganhar à do driver. O check antigo (`nvidia-smi-elf-pf-w`) só inspecciona
  `p_flags` e passava com o binário a crashar — **flags ≠ comportamento**.
- Lições **58** e **59** no `AGENTS.md`.

## Verificação

- Reprodução host acima (139 → 0), com o glibc e o binário exactos da ISO.
- `nix build .#checks.x86_64-linux.nvidia-smi-loads` e `…nvidia-smi-elf-pf-w`
  verdes; `nix-instantiate --parse` nos `.nix` tocados.

## Not-tested (honesto)

- **Re-boot no metal**: nenhum; a prova de que o perfil agora serve o binário
  corrigido é a prioridade avaliada + `ignoreCollisions`, não uma observação.
- `--query-gpu` com driver a responder continua AWAITING_HW.
- Nenhuma ISO foi reconstruída/regravada nesta sessão.
