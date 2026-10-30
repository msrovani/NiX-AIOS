# Córtex/Hermes — manifesto de autonomia (NixAIOS)

**Estado:** doutrina. O prompt que o modelo vê é curto (`/etc/nosd/SOUL.md`);
este documento é para humanos e para o planeamento, **não** para o contexto do
LLM (0.8B, 4k de contexto — um manifesto de 3 páginas come o orçamento e
degrada a resposta).

Regra-mãe: **o manifesto não pode prometer o que o sistema não tem.** Se o
prompt afirma BitNet, wasmtime ou auto-rebuild sem HITL, o modelo vai afirmá-lo
ao utilizador (violação da ADR-0113).

## 1. O que o prompt original acertou

- Posicionamento: Córtex/Hermes é o núcleo de decisão e manutenção, não uma app.
- Imutabilidade do store como fonte de verdade.
- Rollback por geração.
- Separação S1/S2 e agentes WASM frugais.
- Invariantes explícitos (liveness, determinismo, parsimónia, proveniência).

## 2. O que tem de ser corrigido (factos do repo)

| Prompt original | Realidade NixAIOS |
|---|---|
| BitNet b1.58 / 1.58-bit | **REJECT**. S1 = `score_v0` (keyword/heurística, µs). Modelos = GGUF llama.cpp. |
| `wasmtime`/`extism`, heap 16 MB | **wasmi** Frugal: **256 KiB** (4 páginas), **100 000 fuel**, zero caps por omissão. `wasmtime` path A = REJECT. |
| Shared memory SPSC / NoProto / FlatBuffers | IPC = **Unix socket / TCP** com frames length-prefix + JSON (`nos-ipc`). Zero-copy é desiderato, não facto. |
| `nix system rebuild`, `home-manager` | `nixos-rebuild`; no produto, `nix.propose` → build-vm → `nix.confirm` → helper root. `nixos-rebuild` direto = fora do modelo. |
| Auto-rebuild/rollback autónomo | Ring **desarmado** (S-06). Apply é **two-phase com HITL** (nonce server-authored, TTL 600 s). Rollback = geração anterior, mas decidido por humano. |
| "mapear topologia e garantir continuidade" sem limites | Tudo o que é root passa por `CapGate` + HITL; fail-closed. |
| Latência S1 < 15 ms | `score_v0`/keyword sim (µs); **LLM** TTFT é centenas de ms (0.8B CPU) — não misturar. |
| I3: descarregar inactivo > 300 s | Existe `idle_ticks`/S2 e `unload`; **300 s é política a implementar**, não facto. |
| I4: TrustCache | Não existe com esse nome. Proveniência = **TOFU Ed25519** (`keys/pins.dat`), assinatura de skills + allowlist de publishers, **sha256** no fetch de modelos, **BLAKE3** nos blobs. |
| I1: nunca congelar > 50 ms | O tick do Hermes é 2 s e o HUD faz poll a 2 s. 50 ms é **target** para o loop de eventos, não medição. |

## 3. Invariantes (reformulados, verificáveis)

- **I1 — Liveness:** o loop de eventos não bloqueia em I/O de modelo; chat tem
  orçamento de iterações (`chat_budget`) e S2 é assíncrono. Meta: p95 de
  manuseamento < 50 ms, **medido**, não prometido.
- **I2 — Determinismo:** nenhuma alteração fora de derivações Nix. `/etc`,
  `/usr`, `/bin` intocados; config gerada por módulo; estado em `/var/lib/nosd`.
- **I3 — Parsimónia:** modelos/agentes inactivos são libertados por política
  (`score_v0` + `unload`); TTL explícito e observável no `health`.
- **I4 — Proveniência:** modelos com sha256 pinado (diverge = apaga), skills com
  BLAKE3 + assinatura + allowlist, peers com TOFU; sem assinatura = recusa.

## 4. Fronteira de autonomia

O Córtex pode: observar, planear, propor Nix, pedir HITL, executar leitura,
seleccionar skills/modelos, consolidar memória.
O Córtex **não** pode: aplicar root, abrir o ring, instalar publisher fora da
allowlist, desactivar Landlock, falar em nome do utilizador sem grant.

## 5. Prompt que o modelo vê (`SOUL.md`)

Curto, comportamental, sem números inventados:

```
Você é JARBAS, interface do Córtex/Hermes no NixAIOS.
Fale pt-BR, 2 a 4 frases, tom calmo e técnico; sem floreados.
Regras:
1. Honestidade: sem motor/modelo/rede → diga "EngineAbsent"/"sem rede".
   Nunca invente tokens/s, VRAM, resultados ou capacidades.
2. Você não tem root. Acções de sistema exigem proposta + confirmação
   humana (HITL) com nonce do servidor; nunca as executa sozinho.
3. Nix manda: nada de editar /etc, /usr ou /bin. Mudanças são derivações
   propostas; o daemon valida, constrói e pede confirmação.
4. Use só as ferramentas expostas pelo nosd; não simule tool calls.
5. Memória: cite o que lembra; se não lembra, diga.
S1 = keyword/score_v0; S2 = llama-server local. WASM = wasmi 256 KiB/100k fuel.
```
