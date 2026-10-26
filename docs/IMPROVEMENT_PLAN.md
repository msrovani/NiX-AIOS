# Plano de Melhorias — NixAIOS rumo ao Boot→JARBAS

**Baseline:** `34bf150` + 20 arquivos modificados não versionados (out/2026)  
**Norte:** fechar **M3 (Boot→JARBAS)** com evidência, depois **M4 (metal)** e **M5 (frota)**, conforme [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md) e o [`REMIGRATION_PLAN_LEDGER.md`](REMIGRATION_PLAN_LEDGER.md).  
**Regra de ouro:** nenhuma release mistura *hardening* com feature nova.  
**Status:** aprovado — execução iniciada em 2026-10-26.

---

## Decisões confirmadas (2026-10-26)

| # | Decisão | Escolha |
|---|---|---|
| 6.1 | Ring Nix no curto prazo | **Remover do ISO até 0.6**; só religar com nonce + rollback + VM test |
| 6.2 | Engine GGUF | **`llama-server` persistente como default**, `llama-cli` como fallback |
| 6.3 | Escopo 0.5.4 | **P0 + PATH/QEMU** (não só segurança) |
| 6.4 | Hardware M4 | **3 máquinas disponíveis** (Intel iGPU, NVIDIA, AMD) — matriz viável |

**PR 1 (em execução):** S-01 + S-02 + S-03 + o mínimo de S-05 para o `dispatch` não ficar acessível por HTTP sem token.  
**Ordem:** segurança de identidade → token/rotação → PATH/QEMU → CI → docs.

### Progresso da execução

| Item | Status | Evidência |
|---|---|---|
| S-01 identidade por `SO_PEERCRED` + token TCP | **feito** | `nosd/src/rpc/mod.rs`, `ipc_auth.rs` |
| S-02 deny default; `agent`/`caps` ignorados | **feito** | `nos-core/src/cap.rs`, testes |
| S-03 `security.rotate` + revogação | **feito** | `rpc/security.rs`, teste de rotação |
| S-05: HTTP lab off por default; `/rpc` com token, sem CORS, 400/401/403/415 | **feito** | `main.rs`, `rpc/http_lab.rs`, `jarbas-cage.nix` |
| S-06 ring Nix desarmado (`NOSD_NIX_RING` opt-in) | **feito** | `rpc/nix_wifi.rs`, `ipc_auth.rs` |
| Token-first no Unix (CLI/admin) + sudo no transmute do ISO | **feito** | `authenticate`, `transmute-stage.nix` |
| Q-10 testes determinísticos (S1/memória) | **feito** | 10/10 `nos-core`, 5/5 `nos-memory` |
| Q-01 fmt + clippy + Slint feature | **feito** | `cargo fmt --check`, `clippy --all-features`, `cargo check -p nos-jarbas --features slint-ui` |
| R-01 PATH (nosd + helper root + journal group) | **feito** | `neural-os.nix`, `cortex-nix-ring.nix` (falta VM test) |
| R-02 syntax fail-closed | **feito** | `nix_managed.rs`, testes condicionais |
| S-04 HITL grant (single-use, kind, TTL) | **parcial** | `HitlGrantStore`, `ui.action`, `ipc_auth`; admin legacy `hitl_confirmed` |
| S-07 fetch sanitization + cap + magic | **feito** | `nos-cortex/src/fetch.rs`, testes |
| S-08 suíte de regressão | **em curso** | `ipc_auth` + unit tests HTTP/grant/fetch |
| R-09 ISO/QEMU | **OBSERVED** | ISO 2.7G, `nosd` Started, gate MQEMU PASS; `docs/evidence/QEMU_0.5.6.md` |
| R-05 NixOS VM test | **feito** | `checks.x86_64-linux.nixaios-vm` (health + chat, 61s) |
| Q-03 CI GitHub | **criado** | `.github/workflows/ci.yml`; primeira execução no GitHub |
| E-01 llama-server persistente | **feito (host)** | spawn/health/fallback, `llama_server.rs`; e2e GGUF pendente |
| E-02 sem lock durante IO | **feito** | `Arc<dyn InferenceBackend>`, teste 8 threads |
| E-04 validação GGUF | **feito** | `validate_gguf`, teste header |
| M-01 seg.bin fsync/cap | **feito** | `nos-memory/src/seg.rs`, teste oversized |
| J-01 accept-m3 | **criado** | `scripts/accept-m3.sh` |

---

## 0. Sumário executivo

O projeto tem um núcleo bom e uma documentação honesta, mas o produto (systemd + NixOS + ISO) não está verificável: a fronteira CapGate/HITL é auto-atestada, o HTTP lab é um proxy RPC aberto, o `ai-managed.nix` não é importado, e o serviço `nosd` não encontra `nmcli`/`nix`/`llama`/`whisper` no `PATH`.

A sequência proposta é:

1. **0.5.4 — Trust & Truth (hardening, ~5–7 dias-dev):** fechar P0 de segurança, corrigir PATH, tornar testes determinísticos, criar CI, limpar docs/árvore. Sem feature nova.
2. **0.6.0 — M3 Jarvis-like (~12–18 dias-dev):** motor persistente, voz, Wi-Fi, HITL real com nonce, memória no chat, VM test + QEMU verde, gate estrito.
3. **0.7.0 — M4 metal (~8–12 dias-dev + hardware):** campanha metal com evidência, matriz de hardware, rollback.
4. **0.8.0 — M5 frota (~10–15 dias-dev):** mesh autenticada, CRDT P2P, SoulMirror, offload real, lab 2-nós.
5. **0.9+ — escala:** self-heal, factory, marketplace, MCP, BGE, AirLLM, CUDA/ROCm verificados.

**Decisão mais importante:** o item 2 do sumário da análise (caminho root) só pode ser *religado* depois que a identidade IPC e o HITL por nonce estiverem prontos. Até lá, o ring Nix fica explicitamente desarmado.

---

## 1. Critérios de sucesso

### 1.1 Norte funcional (M3)

Um único comando, num ambiente limpo, deve provar:

```bash
# VM NixOS ou QEMU (ISO 051+)
nosctl health                 # status=ok, landlock != Failed, engine honesto
nosctl chat "oi"              # resposta não vazia; cpu-echo ou GGUF declarado
nosctl route "nvidia pci"     # Decision + slot + honesty
nosctl recall "oi"            # memória persiste entre chamadas
nosctl voice-status           # Ready ou EngineAbsent explícito
nosctl ui-card "HITL test"    # janela + card
# jarbas-cage ativo; sem X11; log/journal capturado
```

### 1.2 Critérios de release

Uma release só é promovida se **todos** forem verdadeiros:

- [ ] `cargo fmt --check` limpo.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` limpo.
- [ ] `cargo test --workspace` determinístico (10 execuções consecutivas, 0 falhas).
- [ ] `nix build .#iso` reproduzível; SHA-256 publicado.
- [ ] `nixosTest` de boot + health + chat passa.
- [ ] Gate v2 passa sem `SKIP` nos estágios de aceite.
- [ ] Nenhum P0 aberto (segurança, root, auth).
- [ ] Evidência anexada: commit, ISO SHA, gate JSON, journal, fotos/vídeo metal.
- [ ] Ledger atualizado com status + evidência.
- [ ] Nenhuma afirmação `VERIFIED` sem artefato.

### 1.3 Definição de “DONE” (taxonomia ADR-0113, refinada)

| Status | Exige |
|---|---|
| **IMPLEMENTED** | Código + teste unitário/contrato no host |
| **INTEGRATED** | Serviço real (systemd) + teste de integração no ambiente correto |
| **OBSERVED** | Execução real (VM/QEMU/metal) + log/journal |
| **VERIFIED** | OBSERVED + repetição (≥3x) + artefato versionado + revisão |
| **AWAITING_HW** | Só para dependência física; nunca para bug de software |

Hoje o ledger usa DONE/PARTIAL/NOT_DONE. A proposta é adicionar `INTEGRATED` e `OBSERVED` para eliminar a ambiguidade de “DONE (host)”.

---

## 2. Princípios não negociáveis

1. **Identidade não é payload.** `agent` do cliente nunca define permissão. UID/peer cred + token.
2. **HITL é server-authored.** Confirmação humana é um nonce emitido pelo servidor, de uso único, com TTL e kind; nunca um booleano.
3. **Fail-closed.** Ausência de identidade, de ferramenta ou de parse = negação, não “structural OK”.
4. **Nenhum caminho root a partir de IPC não autenticado.** Ring Nix desarmado até S-01/S-04 verdes.
5. **Nada de comandos nus em serviços.** `Command::new` absoluto ou `systemd.services.<x>.path` explícito.
6. **Teste no ambiente do produto.** Host test não substitui VM/QEMU/metal.
7. **Sem seed de blobs na ISO** (S1.2 permanece REJECT).
8. **Sem Ring0, sem X11, sem wasmtime path A, sem BitNet/Falcon3 nativo.**
9. **Docs = código.** README/API/DEVELOPMENT não podem descrever o que não existe.
10. **Uma release, um objetivo.** Hardening não vira feature; feature não vira “hardening de passagem”.

---

## 3. Modelo de ameaça e zonas de confiança

| Zona | Quem | Pode | Não pode |
|---|---|---|---|
| **Root** | systemd, helpers `nosd-*` | Aplicar Nix, montar, systemctl | Ser acionado por payload não assinado |
| **nosd** | daemon | Inferência, memória, mesh, fetch, UI broker | `nixos-rebuild` direto; escrever fora do allowlist |
| **Kiosk/JARBAS** | processo `nosd` (cage/chromium) | Abrir/fechar UI, submeter HITL com nonce | `NIX_WRITE`, `AGENT_SPAWN`, shell root |
| **Usuário local** | `nixos`, outros logins | CLI `nosctl` se autorizado por grupo | Admin implícito por `agent` |
| **LAN** | peers mesh | HB/FRAG/CRDT autenticados | Eleger Master ou injetar saúde sem chave |
| **WAN** | HF/HTTPS | Fetch deliberado | Seed na ISO, download sem HITL/limite |

**Invariantes de teste:**
- Request sem identidade → `deny`, nunca `LocalAdmin`.
- `agent:"hermes"` via IPC → `deny`.
- `hitl_confirmed:true` sozinho → `deny` em `nix.apply`/`wifi.connect`/`transmute`/`self.heal`.
- `POST /rpc` sem token → `401/403`; com `Origin` externo → sem `ACAO:*`.
- `filename:"../x"` → `deny`/sanitizado.
- `nix-instantiate` ausente → `syntax_ok:false`.

---

## 4. Trilhas de trabalho (workstreams)

Legenda: **P** = prioridade (P0/P1/P2), **E** = esforço (S ≤ 0,5d; M 1–2d; L 3–5d), **Dep** = dependências.

### WS-S — Segurança e fronteira de confiança

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| S-01 | P0 | M | Identidade por `SO_PEERCRED` no Unix socket; mapa uid→perfil; TCP exige token | — | Teste: uid `nixos` ≠ admin; uid `nosd` = perfil kiosk; sem `agent` = deny |
| S-02 | P0 | S | Remover `default_agent="nosctl"`; vazio/ausente = `Unknown`/zero; `hermes` não é perfil IPC | — | Testes de regressão em `nos-ipc`/`nos-core` |
| S-03 | P0 | S | `cap_gen` real (bump em reload de política) ou remover campo + promessa | S-01 | Teste: geração antiga revoga; doc alinhada |
| S-04 | P0 | L | HITL por nonce: `WindowBroker` emite `confirm_id` (kind, TTL, single-use); `ui.action` consome; `nix.apply`/`wifi.connect`/`transmute`/`self.heal` exigem | S-01 | Replay negado; nonce expirado negado; `hitl_confirmed` removido do wire |
| S-05 | P0 | M | HTTP lab: off por default; token 0600; sem `ACAO:*`; 400 em JSON inválido; limite de corpo/tempo; 1 req/conexão | S-01 | Testes HTTP: sem token 401; Origin externo sem CORS; body > limite 413 |
| S-06 | P0 | S | Kill switch do ring Nix: `NOSD_NIX_RING=off` default; só arma com nonce + feature flag | S-04 | `nix.apply` retorna `ring_disabled` até S-04 verde |
| S-07 | P0 | S | Sanitizar `filename` (`file_name`), limite de bytes, magic GGUF, escrita atômica | — | Teste path traversal e download > cap |
| S-08 | P1 | M | Suíte de regressão de segurança (identity, HITL, CORS, traversal, replay) | S-01..07 | Roda no CI; falha se invariante quebrar |
| S-09 | P1 | S | Socket `0660` grupo `nosd`; `RuntimeDirectory` correto; `SocketMode` se socket-activation | R-05 | `stat` verificado no VM test |
| S-10 | P1 | M | Audit log append-only (JSONL): who/what/when/nonce, sem segredos; rotação | S-04 | Consulta `nosctl audit --since` |

**Saída da WS-S:** nenhum caminho privilegiado sem identidade + nonce; HTTP fechado; ring desarmado ou seguro.

---

### WS-R — Runtime, systemd e NixOS

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| R-01 | P0 | M | Auditoria de `Command::new`: `path` explícito em `services.nosd` e helpers; absolutos onde possível; lint `rg 'Command::new\("'` no CI | — | `nosctl doctor` vê nmcli/nix/llama/whisper/piper/tar/util-linux |
| R-02 | P0 | S | `run_syntax_check` fail-closed; `nix-instantiate` absoluto; schema com allowlist real | R-01 | Ausente → `syntax_ok:false`, `check_tool:"absent"` |
| R-03 | P0 | M | Decidir `ai-managed.nix`: (a) remover feature/docs **ou** (b) importar com schema + rollback + nonce | S-04 | ADR escrita; teste VM muda hostname e reverte |
| R-04 | P0 | M | Helper root: `nixos-rebuild` absoluto, target fixo, staging sob dataDir, `build-vm` primeiro, rollback em falha, lock, timeout | R-01/02 | Teste VM: apply ok + rollback ok |
| R-05 | P0 | L | `nixosTest` de fumaça: boot, socket perms, health, chat, route, recall, voice-status, sem X11 | R-01 | `nix build .#checks.x86_64-linux.nixaios-smoke` |
| R-06 | P1 | M | Revisão de sandbox: `ProtectSystem`, `ReadWritePaths`, `RestrictAddressFamilies`, `DeviceAllow`, `systemd-journal` | R-05 | Checklist em `docs/SECURITY.md`; teste de escrita negada |
| R-07 | P1 | M | ISO reproduzível: sem `sed` mutando árvore; manifesto commit/volid/sha; paths relativos | — | `nix build .#iso` + `sha256sum` + manifesto |
| R-08 | P1 | M | `nosctl doctor`: tools, PATH, perms, engine, socket, landlock, mesh; JSON | R-01 | Gate usa o doctor; CI smoke |
| R-09 | P0 | M | Triagem do `nosd.service FAILED` no QEMU: rebuild 051, serial, journal, correção | R-01/R-05 | 5/5 boots com `nosd active` + health |
| R-10 | P1 | M | Automação QEMU: timeout, captura serial, exit code, artefato | R-09 | `scripts/qemu-smoke.sh` no CI noturno |

**Saída da WS-R:** o serviço real encontra suas ferramentas; o ISO boota com `nosd` saudável; ring Nix é seguro ou removido; VM test no CI.

---

### WS-E — Motor de inferência

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| E-01 | P1 | L | `llama-server` persistente (start/stop/health/stream/cancel/ctx reuse); `llama-cli` fallback | R-01 | p95 de chat GGUF documentado; cancel funciona |
| E-02 | P1 | M | Remover lock através de IO; `spawn_blocking`/thread dedicada; máquina de estados do HotSwap | E-01 | Teste de 8 chats concorrentes sem deadlock |
| E-03 | P1 | S | Descoberta: `NOSD_ENGINE_DIR`, `NOSD_LLAMA_CLI`, PATH, config; `model doctor` | R-01 | `nosctl models` mostra engine real |
| E-04 | P1 | S | Validar GGUF (magic/versão/tamanho/hash); rejeitar truncado | — | Teste com arquivo corrompido |
| E-05 | P1 | M | Orçamento de latência: warmup, n_predict, ctx, timeouts, métricas p50/p95 | E-01 | SLO definido em `docs/OPERATIONS.md` |
| E-06 | P1 | M | S2: fila limitada, backpressure, cancelamento, prioridade | E-02 | Teste de saturação; sem OOM |
| E-07 | P1 | M | Config runtime sem `env::set_var`: struct + reload; testes determinísticos | — | `rg set_var` zero no runtime (só testes isolados) |
| E-08 | P2 | M | AirLLM/mmap real + ROCm/OpenVINO path-detect honesto | E-01 | `AWAITING` até evidência |

**Saída da WS-E:** um chat GGUF com latência aceitável, concorrência segura e cancelamento; fallback `cpu-echo` honesto.

---

### WS-J — JARBAS / M3 (produto)

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| J-01 | P0 | M | Script de aceite M3 (QEMU/VM): health→chat→route→recall→voice→ui.card; exit code | R-05/R-09 | `bash scripts/accept-m3.sh` verde |
| J-02 | P1 | L | Cards HITL reais: nix diff, wifi, blob, self-heal; render Slint; submit/cancel/expire | S-04 | Nonce visível na UI; replay negado |
| J-03 | P1 | M | Wi-Fi: `nmcli` no PATH, scan com cache, HITL, erros claros | R-01/S-04 | VM: scan + connect simulado |
| J-04 | P1 | L | Voz: caminhos absolutos, modelos, record→stt→chat→tts→play, estágios, timeouts | R-01/E-01 | Teste com WAV fixture; p95 |
| J-05 | P1 | M | Memória no chat: recall no prompt, remember pós-chat, escopo, PII | — | `nosctl recall "oi"` após chat |
| J-06 | P1 | M | Ciclo kiosk: hotswap→cage, fallback software, chvt, restart limit, logs | R-09 | QEMU: cage ativo 5/5 |
| J-07 | P2 | M | Persona/UX: saudação, boot, estados de erro, cards ricos | J-02 | Roteiro manual + screenshots |
| J-08 | P1 | L | Transmute: nonce, progresso, resume, checksum, disco, cancelamento | S-04/E-01 | Teste de interrupção/retomada |
| J-09 | P1 | M | Firstboot: validação, resume, não-interativo, segredos, logs | R-04 | Teste de respostas inválidas |

**Saída da WS-J:** M3 fecha com evidência, sem “SKIP verde”; JARBAS sobe e conversa.

---

### WS-M — Memória, malha e frota

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| M-01 | P1 | M | nsgdb: fsync, lock, cap de registro, reparo, teste de corrupção | — | Crash test; recall após restart |
| M-02 | P2 | L | L4 real + CRDT/seg; migração de stub | M-01 | Teste de upgrade L4 |
| M-03 | P2 | L | Mesh auth: Ed25519/Noise, TOFU com chave, replay window | S-01 | Peer sem chave não entra; spoof falha |
| M-04 | P2 | L | CRDT P2P + anti-entropy | M-03 | Lab 2-nós converge |
| M-05 | P2 | M | SoulMirror/fleet HUD 2-nós; partição; recuperação | M-04 | `MESH_PARTITION` test |
| M-06 | P2 | L | Offload S2 remoto real (não só hint) | M-04/E-01 | Latência + cancel remoto |
| M-07 | P3 | M | BGE embeddings + rerank opcional; fallback lexical honesto | M-01 | Recall semântico com blob runtime |

**Saída da WS-M:** frota autenticada, memória durável, CRDT real; sem overclaim de “mesh brain”.

---

### WS-O — Observabilidade, gate e honestidade

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| O-01 | P0 | L | Gate v2: JSON, estágios obrigatórios, `SKIP` reprova no aceite, evidência, commit/ISO SHA | R-05/R-09 | `gate --strict` exit 1 se M3 incompleto |
| O-02 | P1 | M | Ledger automatizado: gate/CI atualiza status; bloqueia `VERIFIED` sem artefato | O-01 | PR falha se claim sem evidência |
| O-03 | P1 | M | Logs estruturados + correlation id; sem segredos; retenção | — | `journalctl -o json` parseável |
| O-04 | P1 | M | `nosctl doctor` + `nosctl audit` (com R-08/S-10) | R-08 | Gate usa doctor |
| O-05 | P1 | M | Evidence bundle por release: commit, ISO SHA, gate JSON, journal, fotos, checklist | O-01 | `dist/evidence-<ver>/` |
| O-06 | P2 | S | Lint de honestidade: docs vs ledger; proíbe “VERIFIED” sem link | O-02 | CI docs |

**Saída da WS-O:** o gate vira aceite real; a honestidade deixa de ser prosa e vira verificação.

---

### WS-Q — Qualidade, CI e release

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| Q-01 | P0 | S | CI lint: `fmt --check`, `clippy -D warnings` | — | PR bloqueado |
| Q-02 | P0 | S | CI test: unit + contract; Linux e Windows; determinístico | Q-10 | 10/10 runs |
| Q-03 | P0 | L | CI Nix: `nix build .#iso`, `nixosTest`, cache | R-05/R-07 | Pipeline < 45 min |
| Q-04 | P1 | S | Shellcheck nos scripts | — | Zero erros |
| Q-05 | P1 | S | Secret scan + dependabot/audit | — | Zero segredos |
| Q-06 | P2 | M | Cobertura focada (core/IPC ≥ 80%) | Q-02 | Relatório |
| Q-07 | P2 | M | Fuzz: framing, parsers, HTTP | S-05 | Corpus + crash-free 1h |
| Q-08 | P1 | M | Release automation: tag, changelog, artefatos, checksum | O-05 | `make release VERSION=x` |
| Q-09 | P1 | S | Branch protection + review obrigatório | Q-01 | Config versionada |
| Q-10 | P0 | S | Testes determinísticos: injetar config em vez de `set_var`; `serial` onde inevitável | — | `cargo test -p nos-core` 10/10 |

**Saída da WS-Q:** nenhuma release sem lint/test/build/VM; flakiness zero.

---

### WS-D — Documentação, limpeza e UX

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| D-01 | P0 | S | Corrigir `GETTING_STARTED.md` (`C:\DEV\nix-aios\nix-aios\...` → caminho real) | — | Comando copiável funciona |
| D-02 | P0 | S | Corrigir `DEVELOPMENT.md` (`server.rs` → `rpc/`) | — | Doc = código |
| D-03 | P0 | S | `.gitattributes` (`* text=auto eol=lf` para `.sh/.nix`) | — | `git ls-files --eol` consistente |
| D-04 | P0 | M | Remover paths pessoais (`C:\DEV`, `/mnt/c/Users/msrov`, `$HOME/src`) | — | `rg msrov` zero |
| D-05 | P1 | S | Arquivar/remover scripts s5xx e duplicados; um build canônico | — | `scripts/` enxuto |
| D-06 | P0 | S | Licença completa AGPL + SPDX `AGPL-3.0-only` | — | GitHub detecta |
| D-07 | P0 | S | README: seção “Status real” (scaffold vs produto) com link para gate/ledger | — | Sem overclaim |
| D-08 | P0 | S | API.md: auth, HITL-nonce, ring desarmado, limites | S-04 | Doc revisada |
| D-09 | P1 | M | ADRs 0119–0127 (ver §11) | — | Aceitas |
| D-10 | P1 | S | Link check + spellcheck de docs | — | CI docs |

**Saída da WS-D:** um novo contribuidor consegue rodar, entender o que é real e não se machucar.

---

### WS-H — Hardware e metal (M4/M5)

| ID | P | E | Tarefa | Dep | Aceite |
|---|---|---|---|---|---|
| H-01 | P1 | M | Matriz mínima: 3 máquinas (Intel iGPU, NVIDIA, AMD/ROCm), Wi-Fi, webcam, áudio | J-01 | Tabela preenchida |
| H-02 | P1 | S | Escrita USB (DD) + verificação de volid + checksum | R-07 | 3 sticks verificados |
| H-03 | P1 | M | Preflight: firmware, DRM, NM, áudio, câmera, espaço, TPM/Secure Boot | — | `metal-preflight.sh` |
| H-04 | P1 | M | Captura de boot metal (journal + foto/vídeo + `nosctl doctor`) | H-03 | Evidence bundle |
| H-05 | P1 | M | Thermal/suspend/resume; política de ngl; OOM | H-01 | 24h soak |
| H-06 | P1 | M | Rollback e recuperação (generation, stick B) | R-04 | Teste de brick recovery |
| H-07 | P1 | S | Repetibilidade: 3 boots iguais na mesma máquina | H-04 | 3/3 |
| H-08 | P2 | M | CUDA opt-in verificado (system_info + offload) | E-01 | Evidência, não `nvidia-smi` |
| H-09 | P2 | M | Lab 2-nós físico | M-05 | A/B 0081 |
| H-10 | P2 | M | Frota 3+ nós e falha de peer | M-06 | Convergência |

---

## 5. Fases, sequência e gates

### Fase 0 — 0.5.4 “Trust & Truth” (hardening)

**Objetivo:** parar o sangramento. Nenhuma feature nova.  
**Escopo:** S-01..S-07, R-01, R-02, R-04 (kill switch), R-09, Q-01, Q-02, Q-10, D-01..D-04, D-06..D-08.  
**Duração:** 5–7 dias-dev.

**Entrada:** baseline atual.  
**Saída (gate DG0):**
- [ ] Request sem identidade/vazio/`hermes` → deny (teste).
- [ ] `hitl_confirmed` não é mais aceito sozinho.
- [ ] HTTP lab desligado por default; token; sem CORS `*`.
- [ ] `nix.apply` desarmado (`ring_disabled`).
- [ ] `run_syntax_check` fail-closed.
- [ ] `filename` sanitizado + cap.
- [ ] `systemd.services.nosd.path` completo; `nosctl doctor` vê as ferramentas.
- [ ] QEMU rebuildado; `nosd` ativo; journal anexado.
- [ ] CI lint/test verde; `cargo test` 10/10.
- [ ] Docs drift corrigido; árvore limpa; tag `v0.5.4`.

**Risco:** quebrar o fluxo `nosctl` que dependia de `hitl_confirmed`; mitigar com compat temporária + warning + remoção na 0.6.

---

### Fase 1 — 0.6.0-alpha “Truth on host & ISO”

**Objetivo:** paridade host/ISO; o produto executa o que o host testa.  
**Escopo:** S-08..S-10, R-03, R-05..R-08, R-10, O-01, O-03, E-02, E-03, E-07, Q-03..Q-05, D-05, D-09.  
**Duração:** 7–10 dias-dev.

**Saída (gate DG1):**
- [ ] `nixosTest` de fumaça no CI.
- [ ] Gate v2 estrito; `SKIP` reprova nos estágios de aceite.
- [ ] ISO reproduzível + manifesto.
- [ ] Ring Nix decidido: removido **ou** importado com nonce + rollback + teste VM.
- [ ] Sandbox revisado e documentado.
- [ ] `nosctl doctor` no gate.
- [ ] Nenhum `Command::new` nu em serviço.

**Decisão de arquitetura nesta fase:** ADR-0123 (ring Nix) e ADR-0124 (engine).

---

### Fase 2 — 0.6.0 “M3 Jarvis-like”

**Objetivo:** fechar M3 com evidência.  
**Escopo:** E-01, E-04..E-06, J-01..J-06, J-08, J-09, M-01, O-02, O-04, O-05.  
**Duração:** 12–18 dias-dev.

**Saída (gate DG2):**
- [ ] `bash scripts/accept-m3.sh` verde em VM + QEMU.
- [ ] Chat GGUF real (se modelo presente) ou `cpu-echo` declarado; latência p95 medida.
- [ ] Voz `Ready` real ou `EngineAbsent` com motivo; teste de WAV.
- [ ] Wi-Fi scan/connect no VM.
- [ ] HITL cards reais; nonce; replay negado.
- [ ] Memória persiste e é recuperada no chat.
- [ ] Cage ativo 5/5 no QEMU.
- [ ] Gate JSON + evidence bundle.
- [ ] Ledger: M3 itens `OBSERVED`.

**Critério de “não release”:** se qualquer item de M3 depender de `SKIP`, não é 0.6.0.

---

### Fase 3 — 0.7.0 “M4 metal”

**Objetivo:** provar em hardware.  
**Escopo:** H-01..H-08, R-06, remediações do metal.  
**Duração:** 8–12 dias-dev + acesso a hardware.

**Saída (gate DG3):**
- [ ] 3 máquinas × 3 boots com `nosd` + cage + chat.
- [ ] Evidence bundle por máquina (journal, doctor, fotos/vídeo, SHA).
- [ ] Rollback testado (brick recovery).
- [ ] 24h soak sem restart loop/OOM.
- [ ] `M4` no ledger = `VERIFIED` (não `OBSERVED`).

**Risco:** hardware indisponível. Mitigação: H-01 define mínimo; sem ele, não promover M4.

---

### Fase 4 — 0.8.0 “M5 frota”

**Objetivo:** rede autenticada e memória distribuída.  
**Escopo:** M-02..M-06, H-09, H-10.  
**Duração:** 10–15 dias-dev.

**Saída (gate DG4):**
- [ ] Peer sem chave não entra.
- [ ] CRDT 2-nós converge; partição recupera.
- [ ] SoulMirror no HUD.
- [ ] Offload S2 remoto com cancelamento.
- [ ] Lab 2-nós documentado.

---

### Fase 5 — 0.9+ “Escala”

**Escopo:** M-07, self-heal com aprovação, factory real, marketplace assinado, MCP, AirLLM, ROCm/CUDA, multi-GPU.  
**Regra:** cada item só entra com aceite + evidência; sem “DONE (host)”.

---

## 6. Caminho crítico e paralelização

```
S-01/S-02/S-04/S-05
        │
        ▼
R-01 (PATH) ──► R-02 (syntax) ──► R-05 (VM) ──► O-01 (gate) ──► J-01 (M3 accept)
        │                                              │
        ├──► E-02/E-03 ──► E-01 (llama-server) ────────┤
        │                                              ▼
        └──► S-07 (fetch)                        J-04 (voice) ──► H-01 (metal) ──► M-03 (mesh auth)
```

**Paralelizável desde o dia 1:**
- WS-Q (CI/lint/test) e WS-D (docs/limpeza) com WS-S.
- WS-R (PATH/VM) depois de S-01/S-05.
- WS-E (engine) depois de R-01.
- WS-M (memória) em paralelo com WS-J, sem tocar auth.

**Serial (não paralelizar):**
- S-04 (HITL nonce) antes de religar R-03/R-04.
- O-01 antes de declarar M3.
- M-03 antes de M-04/M-05/M-06.
- H-01 antes de M4.

---

## 7. Estratégia de testes

| Nível | O que cobre | Onde roda | Gate |
|---|---|---|---|
| **Unit** | `nos-core`, parsers, policy, hashing | host/CI | PR |
| **Contract** | `Request.required_cap` × dispatch × handlers | host/CI | PR |
| **Property/fuzz** | framing, GGUF, HTTP, args | CI noturno | semanal |
| **Integration** | daemon real + nosctl + memória + UI broker | host/CI | PR |
| **NixOS VM** | systemd, PATH, perms, socket, ring | `nixosTest` | merge |
| **QEMU/ISO** | boot, cage, chat, transmute, voice | noturno | release |
| **Metal** | hardware, DRM, Wi-Fi, áudio, câmera | manual/3x | release |
| **Security** | identity, nonce, CORS, traversal, replay | CI | PR |
| **Soak** | 24h, restart, memory, OOM | noturno | release |

**Regra:** todo bug corrigido ganha um teste no nível **mais alto** em que ele ocorreu. Bug de PATH não se prova com unit test.

---

## 8. CI proposto (esboço)

```yaml
name: ci
on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings

  test:
    strategy: { matrix: { os: [ubuntu-latest, windows-latest] } }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace
      - run: cargo test -p nos-core -- --test-threads=1   # temporário até Q-10

  nix:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: cachix/install-nix-action@v27
      - run: nix build .#iso --print-out-paths
      - run: nix build .#checks.x86_64-linux.nixaios-smoke
      - run: sha256sum result/iso/*.iso | tee iso.sha256

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: rg -n 'Command::new\("' packages/nosd/crates || true   # policy check
      - run: bash scripts/security-regression.sh

  shellcheck:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: shellcheck scripts/*.sh modules/transmute/*.sh
```

**Branch protection:** lint + test + nix + security obrigatórios; review de 1 pessoa; sem merge em vermelho.

---

## 9. Gate v2 — especificação

**Objetivo:** transformar o gate de “checklist permissivo” em aceite real.

- Saída por linha: `stage=… ring=… layer=… claim=… result=… evidence=…`.
- `result` ∈ `PASS|FAIL|SKIP|BLOCKED`.
- `--strict`: `SKIP` em estágio obrigatório = `FAIL`.
- Estágios obrigatórios por release:
  - 0.5.4: `S0` (security invariants), `R0` (PATH/doctor), `Q0` (lint/test).
  - 0.6.0: `M0–M3`, `MQEMU`, `VM`.
  - 0.7.0: `M4` + metal evidence.
  - 0.8.0: `M5` + fleet.
- Evidência obrigatória: caminho de arquivo/log + hash; sem evidência, `FAIL`.
- O gate escreve `gate.json` com commit, ISO SHA, timestamp, host, resultado.
- `gate --ledger` falha se o ledger tiver `VERIFIED` sem evidência correspondente.

**Mudança cultural:** `SKIP` deixa de ser “ok, não aplicável” e vira “não aceito”.

---

## 10. Métricas e scorecard

| Dimensão | Métrica | Meta 0.5.4 | Meta 0.6.0 | Meta 0.7.0 |
|---|---|---|---|---|
| Segurança | RPC privilegiado sem identidade | 0 | 0 | 0 |
| Segurança | Ações HITL sem nonce | 0 | 0 | 0 |
| Segurança | `Command::new` nu em serviço | 0 | 0 | 0 |
| Confiabilidade | Boot QEMU com `nosd` ativo | 5/5 | 5/5 | 5/5 |
| Confiabilidade | Boot metal com chat | — | — | 3/3 máquinas |
| Qualidade | Testes determinísticos | 10/10 | 10/10 | 10/10 |
| Qualidade | Clippy/fmt | verde | verde | verde |
| Desempenho | Chat p95 (cpu-echo) | < 100 ms | < 100 ms | < 100 ms |
| Desempenho | Chat p95 (GGUF 7B) | — | medido | < 15 s |
| Desempenho | Voz turn p95 | — | medido | < 8 s |
| Honestidade | `VERIFIED` com artefato | 100% | 100% | 100% |
| Honestidade | Gate strict sem SKIP | — | 100% | 100% |

---

## 11. ADRs a escrever

| ADR | Tema |
|---|---|
| 0119 | Identidade IPC: peer creds + token + perfis |
| 0120 | HITL server-authored: nonce, TTL, single-use, audit |
| 0121 | HTTP lab: off by default, token, sem CORS, limites |
| 0122 | Política de `PATH`/exec absoluto em serviços NixOS |
| 0123 | Ring Nix: importar `ai-managed.nix` com schema/rollback **ou** remover |
| 0124 | Engine: `llama-server` persistente vs `llama-cli` por request |
| 0125 | Mesh: autenticação/TOFU com chave, não timer |
| 0126 | Taxonomia de evidência (`IMPLEMENTED`→`INTEGRATED`→`OBSERVED`→`VERIFIED`) |
| 0127 | Trens de release 0.5.4/0.6/0.7/0.8 e critérios de gate |

---

## 12. Riscos e mitigações

| Risco | Impacto | Mitigação |
|---|---|---|
| Root RCE via ring Nix | Crítico | S-01/S-04 antes de R-03; kill switch; nonce; ADR |
| Identidade spoof | Crítico | S-01/S-02; testes de regressão |
| HTTP lab drive-by | Alto | S-05; off default; token; sem CORS |
| PATH quebra ISO | Alto | R-01/R-05; `doctor`; VM test |
| `nosd` FAILED no QEMU | Alto | R-09; journal; rebuild 051 |
| Flakiness | Médio | Q-10; injeção de config |
| Hardware indisponível | Médio | H-01 mínimo; não promover M4 sem |
| Scope creep | Médio | uma release/objetivo; non-goals |
| Bus factor | Médio | docs/ADRs; CI reproducible |
| Build lento | Baixo | cache Nix/Cargo; jobs paralelos |
| Evidência stale | Médio | O-05; SHA + commit no gate |

---

## 13. Non-goals explícitos (até 0.8)

- Ring0/BAREMETAL_ONLY, Limine, IDT, MMIO.
- X11 no kiosk.
- wasmtime como path A.
- BitNet/Falcon3 ternário nativo.
- Seed de GGUF/AgentJev/Piper na ISO.
- Self-heal autônomo sem HITL.
- Marketplace/MCP pleno antes de auth/mesh.
- “Remote infer” antes de M-06.
- Marcar M4/M5 como VERIFIED sem evidência.

---

## 14. Primeiros 10 passos (esta semana)

1. Criar branch `hardening/0.5.4`; congelar features.
2. Escrever testes que **falham hoje**: sem `agent`, `agent:""`, `agent:"hermes"`, `hitl_confirmed` sozinho, `filename:"../x"`, `POST /rpc` sem token.
3. Implementar S-01/S-02 (peer creds + deny default) e S-03 (cap_gen real ou remoção).
4. Implementar S-04 (nonce) e S-06 (ring desarmado).
5. Implementar S-05 (HTTP lab) e S-07 (fetch).
6. R-01: `path` do `services.nosd` + `nosctl doctor`; R-02 fail-closed.
7. Q-10: remover `env::set_var` dos testes/runtime; 10/10 runs.
8. Q-01/Q-02: CI lint/test; corrigir `eq_op` e `cargo fmt`.
9. R-09: rebuild ISO 051, QEMU com serial, capturar journal, corrigir `nosd`.
10. D-01..D-04/D-06..D-08; limpar árvore; tag `v0.5.4` com evidence bundle.

**Comandos de aceite da semana:**

```bash
cd packages/nosd
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
for i in $(seq 1 10); do cargo test --workspace || exit 1; done
nix build .#checks.x86_64-linux.nixaios-smoke
bash scripts/qemu-smoke.sh
bash scripts/gate-boot-to-jarbas.sh --strict
```

---

## 15. Mapeamento para o plano original (ledger)

| Ledger | Tema | Fase | Como fecha |
|---|---|---|---|
| I.1 | CapGate server-authored | 0.5.4 | S-01/S-02/S-03 + testes |
| I.3 | Landlock/systemd real | 0.6.0 | R-06 + VM test |
| NX.0 | Matriz de policy | 0.5.4 | S-04 + testes de disposition |
| NX.1 | ai-managed/rollback | 0.6.0 | R-03/R-04 + VM test |
| NX.3 | Observe journal/NM | 0.6.0 | R-01/R-06 + group journal |
| NX.4 | Actuators | 0.6.0 | S-04 + path + testes |
| M.1 | FRACK/TOFU/eleição | 0.7.0 | M-03 + lab |
| M.2 | CRDT/nsgdb | 0.8.0 | M-02/M-04 |
| M.3 | SoulMirror/frota | 0.8.0 | M-05/H-09 |
| M.4 | Offload S2 | 0.8.0 | M-06 |
| S1.1 | Decision/keyword | 0.6.0 | E-07 + J-01 |
| S1.2 | Sem seed ISO | sempre | guard + CI |
| S2.2 | llama-server stream | 0.6.0 | E-01 |
| HW.1/2 | Inventário/SelfHeal | 0.7/0.9 | H-03/E-08 |
| T.1–T.3 | Hub/route/fit | 0.6.0 | E-01/E-03 |
| DB.1/2 | nsgdb/OPAR | 0.6.0 | M-01/J-05 |
| F.1–F.3 | Factory/seek | 0.9 | fase 5 |
| A.1/A.2 | Agency/skills | 0.9 | fase 5 |
| U.2/U.3 | Safety/BEI | 0.8.0 | S-10/M-01 |
| B.1 | Early boot | 0.6.0 | R-05/R-09 |
| N.1 | Nix ring | 0.6.0 | R-03/R-04 |
| J.1/J.2 | Voz/UI | 0.6.0 | J-02/J-04 |
| G.1 | Honestidade | contínuo | O-01/O-02/O-06 |
| P.1–P.16 | Produtos | 0.9+ | fase 5 |

---

## 16. Definition of Done por release

### 0.5.4
- [ ] P0 fechados; ring desarmado; HTTP fechado.
- [ ] PATH/doctor; syntax fail-closed; fetch sanitizado.
- [ ] CI lint/test; 10/10; docs limpos; tag + evidence.

### 0.6.0
- [ ] M3 aceito sem SKIP (`accept-m3.sh`).
- [ ] VM + QEMU; cage; chat; voz Ready/Absent; Wi-Fi; memória.
- [ ] HITL nonce; cards; audit.
- [ ] Gate v2 + ledger OBSERVED.

### 0.7.0
- [ ] 3 máquinas × 3 boots; rollback; 24h soak; M4 VERIFIED.

### 0.8.0
- [ ] Mesh auth; CRDT P2P; 2-nós; offload; M5 VERIFIED.

---

## 17. Observações finais

- O plano **não** joga fora o que existe: aproveita o núcleo, o ledger, a UI e a doutrina. Ele muda a ordem: **confiança → verdade de ambiente → produto → hardware → frota**.
- A parte mais barata e de maior impacto é a Fase 0: identidade, nonce, HTTP, PATH, fail-closed, CI. Ela destrava quase tudo.
- A parte mais cara é o motor (E-01) e o metal (H-01). Não comece por eles.
- Se o tempo for curto, faça **S-01/S-02/S-04/S-05 + R-01/R-02 + Q-10** e pare. Isso já transforma o projeto de “scaffold perigoso” em “scaffold confiável”.
- Se só uma coisa for feita: **remova o caminho root do IPC até o HITL por nonce existir.** É a diferença entre um bug e um incidente.
