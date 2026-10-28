# NIXAIOS Remigração — Ledger do plano (fonte de verdade)

**Plano:** Cursor plan `nixaios_remigração_34e29087` (aprovado 2026-10-25).  
**Sessão audit:** SESSION_529.  
**Regra:** UI “todo completed” ≠ aceite do plano. Taxonomia ADR-0113:

| Status ledger | Significado |
|---------------|-------------|
| **DONE** | Código + teste host (ou Nix wired) cobre o *goal* do item |
| **PARTIAL** | Existe scaffold / wire parcial; goal do plano **não** fechado |
| **NOT_DONE** | Overclaim ou ausente — reabrir na próxima onda |
| **PREEXIST** | Já existia antes desta remigração; só referenciado |

Contagens (fecho plano c2d76600 host): **DONE ~18 · PARTIAL ~35 · NOT_DONE ~2 · REJECT 1**.  
**0.5.0/0.5.1 (2026-10-25):** P0 metal pós-Q5 Linux-first + Landlock FAT allow + CRDT/SoulMirror thin + llama_server_posture + SelfHeal deny stub + docs modernas — ship Git [`RELEASE_0.5.1.md`](RELEASE_0.5.1.md). Ondas = host-OK; metal/lab e2e = residual AWAITING_HW (sem overclaim).

**0.5.2 (2026-10-25):** Revisão estrutural A+B+C — archive s5xx, IPC registry, autolearn off, hub único, self.heal RPC, HermesEngine::boot, rpc/ fatiado, firstboot/hotswap externos — [`RELEASE_0.5.2.md`](RELEASE_0.5.2.md). Volid permanece `NIXAIOS051`.

**0.5.4 Trust & Truth (em curso, 2026-10-26):** S-01/S-02/S-03 — identidade IPC por `SO_PEERCRED`/token, `agent`/`caps` legado ignorados, `security.rotate` com revogação; HTTP lab `/rpc` com token/same-origin/limites. Evidência: `nosd/tests/ipc_auth.rs`, `cargo test --workspace`, `clippy -D warnings`. Plano: [`IMPROVEMENT_PLAN.md`](IMPROVEMENT_PLAN.md). Ring Nix continua desarmado até S-04.

**0.5.5 HITL & Fetch (2026-10-26):** S-04 parcial (grant single-use/kind/TTL via ui.action; replay negado; admin legacy hitl_confirmed), S-07 (basename/cap 8GiB/GGUF magic/partial cleanup). Evidencia: `nosd/tests/ipc_auth.rs` (7), `nos-cortex` fetch tests, `cargo test --workspace`, `nix build .#nosd`. Ring Nix continua desarmado.

**0.5.6 QEMU green (2026-10-26):** R-09 OBSERVED - ISO 2.7G volid NIXAIOS051 (sha256 26be2f...), QEMU TCG com nosd Started, nosd-early Finished, JARBAS hub, multi-user; gate MQEMU PASS. Fixes: replaceVars (nixpkgs 26.11), UTF-8 nosd-early, LF em .nix/.sh, console=ttyS0. Evidencia: `docs/evidence/QEMU_0.5.6.md`. R-05 VM test PASS (health+chat, 61s). Q-03 CI criado. QEMU != metal.

**0.6.0-alpha.1 Engine persistent (2026-10-26):** E-01 llama-server spawn/health/fallback; E-02 Arc backend sem lock durante IO; E-04 validacao GGUF; M-01 seg.bin sync/cap; J-01 accept-m3. Evidencia: `nix build .#nosd`, VM test health+chat, `cargo test --workspace` 3x. E2E GGUF/llama-server, voz, cage e Wi-Fi = residual.

**0.6.0-alpha.2 GGUF real VM (2026-10-27):** R-01 corrigido (path em serviceConfig era ignorado), Landlock RO /nix/store/etc/run (subprocessos), llama-server API key+no-webui. VM test com stories15M: llama-server-http, chat, unload/reload PASS. Latencia CPU/TGC ~38s (nao SLO). Evidencia: `docs/evidence/VM_GGUF_0.6.0-alpha.2.md`. Metal/CUDA/voz/cage/Wi-Fi = residual.

**0.6.0-alpha.3 Hot download + voz (2026-10-27):** S-04 blobs/fetch com grant single-use (replay -> HITL), nosctl fetch --confirm-id / ui-open --grant-kind. Voz: voice-stt/voice-tts CLI, VM status Ready + TTS WAV 1.2s; STT jfk 23min TCG (manual). accept-m3 no VM. Evidencia: `docs/evidence/VOICE_0.6.0-alpha.2.md`, `VM_GGUF_0.6.0-alpha.2.md`. Cage/Wi-Fi/metal/S-08/S-09 pendentes.

**0.6.0-alpha.4 Boot→JARBAS Slint cage/DRM (2026-10-27):** M3 visual em QEMU (Ring3_Linux/qemu OBSERVED): `jarbas-cage` active, cage DRM/KMS 1280x800, HUD Slint (`nos-jarbas --ui`, winit-software + WLR_RENDERER=pixman) com screenshot do scanout. Fixes: XDG_RUNTIME_DIR /run/nosd/user (logind escondia /run/user/nosd), PATH com systemctl (waitScript apagava seatd.sock vivo), StartLimit* no unit, cargoBuildFlags (cargoExtraArgs ignorado no nixpkgs 26.11 -> ISO sem slint-ui), panic tokio aninhado, winit->X11, LIBGL_ALWAYS_SOFTWARE. Evidencia: `docs/evidence/CAGE_SLINT_0.6.0-alpha.4.md`. Metal/GL real, Wi-Fi, S-08/S-09, ring e M4/M5 pendentes.

**0.6.0-alpha.5 Boot→firstboot→JARBAS headless (2026-10-27):** a ISO completa boot → firstboot automático → transmute → hotswap → jarbas-cage/DRM → HUD Slint sem intervenção. Fixes: firstboot em ttyS0/hvc (gatilhos só viam /dev/ttyN), nosd-hotswap-aios sem `path` (exit 127: env bash), hotswap reiniciava o nosd que servia o RPC (nosctl exit=1), transmute tratava systemctl sem root como erro (o .path é o mecanismo), `packages/nosd/target` copiado read-only para a store (nix build após cargo test), chvt antes do cage (1º frame preto em VT1). Evidencia: `docs/evidence/BOOT_FIRSTBOOT_0.6.0-alpha.5.md`. Metal, Wi-Fi, S-08/S-09, ring e M4/M5 continuam pendentes.

**0.6.0-alpha.6 S-08/S-09 hardening (2026-10-27):** socket Unix passa a 0660 (chmod apos bind, aborta se falhar) e suite de regressao alargada: socket 0660, metodo desconhecido fail-closed, admin sem token negado, frame u32::MAX -> TooLarge, required_cap -> DENY_UNKNOWN. Sem features novas. Evidencia: `cargo test --workspace`, `nix build .#iso` (tests em Linux), gate MQEMU. Ring continua desarmado; S-04 legacy e M4/M5 pendentes.

**0.6.0-alpha.7 Ring Nix: decisao desarmado (2026-10-27):** R-03 decisao escrita (ADR addendum 0118-nix-ring-decision): ring fica OFF (armRing=false); religar so com S-04 completo + teste VM apply/rollback + ADR. R-04: o helper root deixou de confiar no pedido - kill switch NOSD_NIX_RING=1, target fixo /etc/nixos/ai-managed.nix, staging confinado ao dataDir, flock, timeout 900/600, PATH com coreutils. VM test prova que um pedido forjado em /var/lib/nosd (1777) e ignorado. Evidencia: `docs/evidence/NIX_RING_0.6.0-alpha.7.md`. R-03/R-04 NAO estao verdes.

**M5 lab: mesh 2-VM (2026-10-27):** novo check `checks.x86_64-linux.mesh-2vm` (dois NixOS, VLAN 1, UDP 19000): `mesh_peers==1` nos dois nos, HB com PK (TOFU pk-bound) e eleicao stub (menor id=Master, outro Worker). TOFU settle 7.2s coberto por `nos-mesh::tofu_settles_after_window`. Sem alteracao de produto (ISO alpha.7). CRDT/SkillSync wire continua residual ("CRDT NOT_DONE" em fleet.health). Evidencia: `docs/evidence/MESH_2VM_0.6.0-alpha.7.md`. M4/M5 metal seguem AWAITING_HW.

**0.6.0-alpha.8 S-04 fechado (2026-10-27):** `hitl_confirmed` deixa de autorizar - nem root/token; a unica prova e `confirm_id` server-authored (ui.open + ui.action, single-use, kind-bound, TTL 5min). Gate SITE_POLICY passa a exigir grant por dominio; nosctl perde `--confirm` e ganha `consent <kind>`; firstboot obtem grant e usa `--confirm-id`. Prova na ISO: firstboot -> grant -> transmute RPC OK -> hotswap -> HUD; e dois negativos (sem grant e grant bogus) recusados no HITL mesmo como root. Bug extra corrigido: nosctl entrava em panico com EPIPE (grep -q/head). Evidencia: `docs/evidence/HITL_0.6.0-alpha.8.md`. Ring continua desarmado.

**SKYNET plano de frota (2026-10-27):** `docs/SKYNET_DISTRIBUTED_PLAN.md` - pesquisa + decisoes para processamento distribuido: TP so intra-host, PP em LAN, remote-prompt em WAN; identidade Ed25519 + Noise IK + NoProto v2 assinado; Merkle-CRDT delta + bloom; JobEnvelope com quotas e resultado assinado; wasmi/WASI para skills e app swap; telepatia = canal de estado (LayaIntent/CRDT/KVREF) em vez de prompt; matriz de ameacas com 12 testes negativos. Fases M6-M10; M10 bloqueada por hardware. Nada implementado nesta entrada.

**M6 identidade de nos (2026-10-27):** implementado no tree (nao tagged): `nos-mesh::ident` com Ed25519 (`ed25519-compact`), id derivado da chave, HB assinado (`KEY\0 pk` + `SIG\0 sig`) com `seq` monotónico, pinagem TOFU, allowlist (`NOSD_MESH_ALLOWLIST`) e recusa de HB sem assinatura (`NOSD_MESH_REQUIRE_SIG=1`, fail-closed se a chave falhar). `health` expoe `mesh_pinned`/`mesh_node_id`. Prova: 8 testes unitarios em `nos-mesh` + check `mesh-2vm-signed` (peers=1, pinned=1, HB forjado sem assinatura ignorado). Evidencia: `docs/evidence/M6_IDENTITY_0.6.0-alpha.8.md`. Falta confidencialidade (Noise) e revogacao; M7-M9 seguem o plano `docs/SKYNET_DISTRIBUTED_PLAN.md`.

**M7 jobs remotos (2026-10-27):** `nos-mesh::job` (frame NJOB assinado), listener TCP `NOSD_JOB_BIND`, RPC `job.submit` e `nosctl job-submit`. O worker tem de ser peer com chave fixada no mesh; o cliente verifica a assinatura do resultado. Limites: prompt 8 KiB, corpo 1 MiB, deadline 0.1-120 s, timeout deadline+2 s. Executor = chat local com gate INFER|MEMORY (sem root). Prova: check `mesh-2vm-jobs` (A sem modelo -> B responde verificado; prompt gigante e porta fechada recusados). Sem Noise (confidencialidade), sem scheduler/fila, sem quota por peer, sem idempotencia. Evidencia: `docs/evidence/M7_JOBS_0.6.0-alpha.8.md`.

**M8 estado replicado (2026-10-27):** CRDT (LWW+VV) transportado no link
autenticado do M7 (`kind=2`, `submit_crdt`, `CrdtHook`); RPC `crdt.put/get/sync`
e `nosctl crdt-put/crdt-get/crdt-sync`. Prova: `mesh-2vm-crdt` - A escreve e
sincroniza para B, B escreve e sincroniza de volta, ambos com 2 entradas.
Anti-entropy v1 envia estado completo (sem delta/Merkle/bloom); em claro (Noise
= M6.1); sem persistencia em nsgdb. Evidencia:
`docs/evidence/M8_CRDT_0.6.0-alpha.8.md`.

**M9 skills wasmi (2026-10-27):** `skill.install` (wat ou wasm_b64),
storage content-addressed em `<data-dir>/skills/<fnv>.wasm`, registo
`skill/<nome>/active` no CRDT, `skill.list`/`skill.run` (+ nosctl). Sandbox
frugal do `nos-wasm`: fuel 100k, heap 256 KiB, zero capacidades (import
`aios_ai::infer` devolve -1). Testes: install/run/list (7==7), fuel bomb
recusada, infer sem cap negado (14 PASS em ipc_auth). Residual: fnv nao e
commitment (BLAKE3 no M8.1), sem SKILL_PUSH/distribuicao do binario, sem quota
por skill, sem Noise. Evidencia: `docs/evidence/M9_SKILLS_0.6.0-alpha.8.md`.

**M6.1 Noise (2026-10-27):** link de jobs/estado passa a `Noise_XX_25519_
ChaChaPoly_BLAKE2s` (estaticas X25519 descartaveis) + `BIND` Ed25519 assinado
sobre `"NJOBBIND"||handshake_hash||pub||id`, verificado contra a chave fixada
no mesh. Sem conversao de chaves nem PKI; sem modo plaintext (nao ha downgrade).
Provas: unit `noise_job_roundtrip` (pinning nos dois lados) e os checks
`mesh-2vm-jobs`/`mesh-2vm-crdt` a passar pelo canal cifrado. Residual: sem
revogacao/rotacao (M6.2), sem teste de MITM activo, HB UDP continua assinado
mas em claro, e o `crdt.sync` envia estado completo (M8.1). Evidencia:
`docs/evidence/M6.1_NOISE_0.6.0-alpha.8.md`.

**M8.1 delta anti-entropy (2026-10-27):** `crdt.sync` deixa de enviar o estado
completo: pull com `version_vector` + push do `delta_since(vv)` (duas trocas,
canal Noise). `CrdtStore::version_vector/delta_since` e
`MeshEngine::crdt_version/crdt_delta_since/crdt_merge_entries`. Prova:
`mesh-2vm-crdt` PASS - a segunda sincronizacao leva so o que falta (k2, nao k1).
Residual: sem bloom/Merkle, sem persistencia em nsgdb, sem compactacao.
Evidencia: `docs/evidence/M8.1_DELTA_0.6.0-alpha.8.md`.

**M9.1 SKILL_PUSH (2026-10-27):** `KIND_SKILL=3` no link (Noise+frame
assinado); `skill.push <addr> <nome>` envia {name,hash,wasm_b64};
`install_bytes(expect_hash)` recusa se o hash do conteudo nao bater (fail-closed)
antes de gravar/activar. `skill-install` aceita .wat ou .wasm. Prova:
`mesh-2vm-skills` PASS - A instala e corre ident(5), empurra para B, B ve na
lista e corre ident(9); push de skill inexistente falha. Residual: hash FNV
(nao commitment; BLAKE3 pendente), 512 KiB/push via base64 no envelope, sem
gossip/revogacao de skills. Evidencia:
`docs/evidence/M9.1_SKILL_PUSH_0.6.0-alpha.8.md`.

**M8.1b durabilidade CRDT (2026-10-27):** log append-only
`<data-dir>/crdt.jsonl` com replay no arranque (merge idempotente) e escrita em
`crdt_put`/`crdt_merge_entries`; abrir o log e fail-closed (se falhar, o nosd
nao arranca). Prova: unit `crdt_log_survives_restart` e `mesh-2vm-crdt` PASS.
Residual: JSONL proprio (nao nsgdb), sem compactacao, sem fsync por op, sem
bloom/Merkle, log nao autenticado em repouso. Evidencia:
`docs/evidence/M8.1b_CRDT_LOG_0.6.0-alpha.8.md`.

**M8.1c compactacao do log (2026-10-27):** `with_crdt_log` tolera linhas
truncadas (crash a meio de write) e compacta acima de 64 linhas: reescreve
`crdt.jsonl.tmp` com uma linha por entrada, `sync_all` + `rename` atomico.
Prova: `crdt_log_compacts_and_tolerates_corruption` (100 puts + linha truncada
-> 1 entrada v99, log <= 2 linhas) e `crdt_log_survives_restart`. Residual:
compactacao so no arranque (falta periodica), sem fsync do diretorio, continua
JSONL (nao nsgdb) e log nao autenticado. Evidencia:
`docs/evidence/M8.1c_CRDT_COMPACT_0.6.0-alpha.8.md`.

**M8.1c fecho (2026-10-27):** compactacao em runtime (`crdt_maybe_compact()`
a cada 60 ticks, contador em `CrdtLog`) e `fsync` do diretorio apos o rename.
Teste `crdt_log_compacts_at_runtime` (100 puts -> compacta -> put -> reopen com
k=v99 e k2=x). Ficam em aberto: nsgdb como backend (M8.1d) e autenticacao do log
em repouso. Evidencia: `docs/evidence/M8.1c_CRDT_COMPACT_0.6.0-alpha.8.md`.

**M8.1e log autenticado (2026-10-27):** cada linha do log CRDT passa a ser
`json + tab + ed25519_sig` com a identidade do no; replay recusa linha sem
assinatura/invalida (fail-closed com identidade) e o compact tambem assina.
Prova: `crdt_log_rejects_tampered_lines` (linha forjada nao substitui a genuina)
+ 40 testes. Fica em aberto: sem deteccao de rollback (falta ancora monotonica),
log antigo sem assinatura e recusado (migracao = apagar/re-assinar), e continua
JSONL (nsgdb = M8.1d). Evidencia:
`docs/evidence/M8.1e_CRDT_LOG_AUTH_0.6.0-alpha.8.md`.

**S1 skills trust (2026-10-27):** hash de skill passa a **BLAKE3** (64 hex) e
cada skill ganha manifesto `skill/<n>/meta` (focus, version, caps, bytes,
provenance). Novo `skill.explain <focus>` pontua 0-100 (proveniencia, caps
minimas, versao, tamanho), escreve `focus/<f>/active` e `why` no CRDT
(auditavel/replicavel). Push marca `provenance=peer`; install `local`.
Prova: `skill_focus_selection_prefers_newer_and_uses_blake3` (hash 64, sum2@2.0.0
vence) + 15 testes em ipc_auth. Residual S2: sem assinatura de publisher, sem
allowlist, sem caps declaradas, sem historico no score, sem versao monotonica.
Evidencia: `docs/evidence/S1_SKILL_TRUST_0.6.0-alpha.8.md`.

**S2a publisher (2026-10-27):** `skill.install` aceita `publisher`+`sig` sobre
`name|focus|version|hash`; `verify_publisher` corre antes de escrever (sem sig =
local; sig invalida = recusa; publisher fora de `NOSD_SKILL_PUBLISHERS` =
recusa; valida = provenance `signed`, +30 no score). Prova:
`signed_manifest_is_accepted_and_tamper_rejected` + workspace verde. Fica para
S2b: caps declaradas, historico no score, versao monotonica, push assinado e
revogacao por CRDT/HITL. Evidencia:
`docs/evidence/S2A_SKILL_PUBLISHER_0.6.0-alpha.8.md`.

**S2b caps + versao monotonica (2026-10-27):** `caps` declaradas passam a ser
validadas contra allowlist fechada (`aios_ai.infer`, `log`) antes de escrever, e
instalar versao inferior a maxima do mesmo `focus` e recusado (fail-closed).
`put_meta` regista as caps. Prova: `declared_caps_are_restricted_to_allowlist`,
`semver_key_orders_versions`, workspace+clippy verdes. Fica para S2c: historico
no score, push assinado, revogacao CRDT/HITL e caminho HITL para downgrade.
Evidencia: `docs/evidence/S2B_SKILL_CAPS_0.6.0-alpha.8.md`.

**0.6.0-alpha.9 frota (2026-10-27):** release que junta M6 (identidade), M6.1 (Noise + BIND), M7 (jobs remotos), M8/M8.1 (CRDT + delta), M8.1b/c/e (log duravel, compactado e assinado em repouso), M9/M9.1 (skills wasmi + push com hash) e S1/S2a/S2b (BLAKE3, manifesto, publisher assinado, caps e versao monotonica). ISO sha256 `ff1f3f53d675ce4639d32ddd26f6a39563bf30302c839c2aaf76ac20a1a86a1b`, gate MQEMU PASS. Residuais: S2c, M8.1d (nsgdb), HB de descoberta em claro, frota real e M4/M5 metal. Evidencia: `docs/RELEASE_0.6.0-alpha.9.md`.

**S2c historico no score (2026-10-27):** `skill.run` passou a registar
`skill/<n>/stats=[ok,fail]` no CRDT e `skill.explain` soma 20*ok/(ok+fail)
quando ha >=5 execucoes (senao 8); `why` mostra `history` e `runs`. Prova:
`skill_history_feeds_the_score` (good 5x ok vs bomb 5x fuel -> good@1.0.0,
history=20, runs=[5,0]) e 16 testes em ipc_auth. Residual: historico local e
sem decaimento, push nao assinado, sem revogacao, sem downgrade HITL.
Evidencia: `docs/evidence/S2C_SKILL_HISTORY_0.6.0-alpha.9.md`.

**S2c autoria no manifesto (2026-10-27):** quando o install e assinado, o
manifesto passa a guardar `publisher` e `sig` (`meta_with_auth`), em vez de
descartar a assinatura apos verificar. Pre-requisito do push assinado (o
destino podera verificar autoria em vez de marcar so `peer`). Prova:
`meta_records_publisher_and_signature_when_signed`. Falta: reencaminhar no
push, revogacao e downgrade HITL. Evidencia:
`docs/evidence/S2C_SKILL_AUTH_IN_META_0.6.0-alpha.9.md`.

**S2c push assinado (2026-10-27):** o emissor inclui focus/version/publisher/sig
no payload (`push_payload`) e o destino verifica com `verify_publisher` ANTES de
gravar: valida = `provenance=signed`, invalida = push recusado, ausente = `peer`.
Prova: `push_payload_carries_authorship` (payload) + `signed_manifest_is_accepted
_and_tamper_rejected` (verificador). **Sem teste e2e da ligacao** (nao ha
assinatura no check de VM). Falta: revogacao e downgrade HITL. Evidencia:
`docs/evidence/S2C_SIGNED_PUSH_0.6.0-alpha.9.md`.

**S2c revogacao (2026-10-27):** `verify_publisher` recebe o conjunto de
publishers revogados (`skill/revoked/<hex>` no CRDT) e recusa **mesmo com
assinatura valida**; aplicado em install e no hook do push. Revogar hoje =
`nosctl crdt-put skill/revoked/<hex> 1` (replica via crdt.sync). Prova:
`revoked_publisher_is_refused_even_with_valid_signature`. Falta CLI/HITL de
revogacao, e2e em VM, expiracao e downgrade HITL. Evidencia:
`docs/evidence/S2C_REVOKE_0.6.0-alpha.9.md`.

**CLI autoria de skills (2026-10-27):** `nosctl skill-sign` (assina
`nome|focus|versao|hash` com a chave do no e imprime publisher+sig),
`skill-install --focus/--version/--publisher/--sig` e `skill-revoke <hex>`
(crdt.put skill/revoked/<hex>). Desbloqueia o e2e do push assinado no check de
VM. Evidencia: `docs/evidence/CLI_SKILL_AUTH_0.6.0-alpha.9.md`.

**S2c e2e push assinado (2026-10-27):** `mesh-2vm-skills` passou a provar a
cadeia completa com o CLI real: install -> skill-sign (chave do no) -> install
com --publisher/--sig -> push -> `provenance=signed` no destino -> revogar ->
push recusado. Assercoes com `json.loads` (sem grep fragil). Isto fecha o
"implementado mas nao provado". Residual: chaves de publicador separadas,
HITL/auditoria na revogacao, expiracao, downgrade HITL e decaimento do
historico. Evidencia: `docs/evidence/S2C_SIGNED_PUSH_E2E_0.6.0-alpha.9.md`.

**S2c downgrade HITL (2026-10-27):** instalar versao inferior a maxima do
`focus` exige grant server-authored `skill.downgrade` (mesmo mecanismo do S-04);
sem grant e recusado. CLI: `skill-install --confirm-id`. Prova:
`skill_downgrade_requires_hitl_grant` (2.0.0 -> 1.0.0 sem grant = recusa; com
grant = aceita) e 17 testes em ipc_auth. Residual: grant nao audita quem
autorizou (S2d) e nao ha e2e em VM deste caminho. Evidencia:
`docs/evidence/S2C_DOWNGRADE_HITL_0.6.0-alpha.9.md`.

**M8.1d anti-rollback (2026-10-27):** ancora `crdt.anchor` (`<bytes> <linhas>`)
ao lado do `crdt.jsonl`, atomica (tmp+rename+fsync do dir) e gravada ao abrir,
ao compactar (antes do rename) e a cada 32 linhas. Arranque recusa log mais
pequeno do que a ancora (fail-closed, mensagem indica o ficheiro a apagar).
Prova: `crdt_log_detects_rollback`, `crdt_log_accepts_growth_and_compaction` e
`crdt_rollback_refuses_start` (daemon real, exit != 0 + stderr "rollback").
Residual: mesma pasta (quem escreve no data-dir repoe os dois), janela de 31
linhas, so deteta encolhimento, backend nsgdb nao feito. Evidencia:
`docs/evidence/M8.1D_CRDT_ANCHOR_0.6.0-alpha.9.md`.

**Boot→JARBAS (playbook M0–M5, 2026-10-26):** [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md) + gate + [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) + [`FLEET_LAB.md`](FLEET_LAB.md) + [`memory/SESSION_BOOT_JARBAS.md`](memory/SESSION_BOOT_JARBAS.md). M3: `route.explain` trinity/`Decision`/S1 LOO + voice stages + **`ui.card` HITL thin**. Gate `MQEMU` = QEMU OBSERVED ≠ metal. **M4/M5** = **AWAITING_HW**.

---

## Tema 0 — Isolamento

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| I.1 | CapGate server-authored; deny client ALL | **DONE** (0.5.4) | `PeerProfile::from_unix_cred`; token-first; `agent`/`caps` ignorados; `security.rotate`; `ipc_auth.rs` (TCP + Unix) |
| I.2 | Host-imports wasmi CapGate + Frugal prove deny | **PARTIAL** | Frugal + `aios_ai::infer` CapGate; fuel trap pré-existente; prove deny INFER OK |
| I.3 | systemd Protect* + Landlock **real** NIX_WRITE | **PARTIAL**↑ | 0.5.0: allowlist +`/mnt/nixaios-log`; `path_allowed` teste FAT; Verified só Linux+ABI; lab metal AWAITING |
| I.4 | Factory B/C PathBcAbsent | **DONE** / PREEXIST | `FactoryError::PathBcAbsent` |
| I.5 | Engine dGPU cgroup ≠ cage iGPU | **PARTIAL**↑ | engine OOM=+200 MemoryMax DevicePolicy=closed (sem DRM); cage DRM+OOM=-500; `lab-oom-engine.sh`; e2e metal AWAITING |

## Tema NX — NixOS Neural Ring

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| NX.0 | Matriz Auto/Confirm/Escalate/Deny; ISO autoSwitch=false | **DONE** | `policy_matrix_prod` + `policy.matrix` RPC; ISO autoSwitch=false |
| NX.1 | Schema ai-managed + build-vm + HitlConfirm + generation/rollback | **DONE** (host) | schema deny-list; `nix.build_vm`; HitlConfirm; buildVmFirst=true default; metal e2e residual |
| NX.3 | nos-observe journald+udev+NM DBus+hwmon→EventBus | **DONE** (host) | journalctl/nmcli/sysfs + `OBSERVE_SNAPSHOT` bus; DBus linkado **NOT** |
| NX.4 | Actuators CapGate NM/PipeWire/GPU/systemctl/DeviceRecipe | **DONE** (host) | systemctl+audio.volume+gpu.plan+device.recipe; NM wifi pré-existente |
| NX.5 | Capacity contínuo; presence; ngl/ROCm/OpenVINO; cage≠engine | **PARTIAL** | Continuous ngl + `accel` cuda/rocm/openvino path-detect; runtime ROCm/OV **NOT** |
| NX.6 | CognitiveAct OPAR universal + seek audit | **DONE** (host) | `seek_audit` em CognitiveAct; factory.seek/autolearn/spawn OPAR; SelfHeal residual |
| NX.7 | PackageHub store federado | **PARTIAL** | `PackageStore` + seek; Ed25519/sandbox militar pleno **NOT** |

## Tema M — Mesh Brain

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| M.1 | FRACK, TOFU settle, CapacityScore, roles, election | **DONE** (host) | `frack`+UDP TX pós-FRAG; `tofu_settled` 130 ticks; election CapacityScore; testes 21/21; lab 2-nó e2e = residual |
| M.2 | nsgdb CRDT + mesh_knowledge FNV + LayaIntent=8 fio | **PARTIAL**↑ | 0.5.0: `nos-mesh::crdt` LWW+VV host tests; LayaIntent TX/RX; **P2P NSGDB sync / lab 2-nó NOT** |
| M.3 | JARBAS HUD fleet + SoulMirror + lab 2+ nós | **PARTIAL**↑ | 0.5.0: `SoulMirrorSnapshot` + bus `SOUL_MIRROR`; lab 2-nó e2e **AWAITING_HW** |
| M.4 | capacity_weighted_assign offload S2 | **DONE** (host hint) | `assign_from_mesh_tuples` + `MESH_OFFLOAD_HINT` bus; remote infer e2e residual |

## Tema S1 / S2 / HW

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| S1.1 | Laya/Jev Decision Intent scorer CPU | **PARTIAL**↑ | `s1_backend_effective`: agentjev sem blob → keyword; `NOSD_S1_BLOB`/models/agentjev.gguf; LOO load metal AWAITING |
| S1.2 | Embutir scorer+Piper seed ISO warm mmap | **REJECT** | Política 2026-10-25: blobs externos **só runtime** via Cortex→Hermes→JARBAS (`blob_deliberate`); ISO NÃO embute |
| S1.2b | Runtime blob/firmware fetch deliberado | **DONE** | `nos-hermes::blob_deliberate` + `model.fetch`/`firmware.fetch`/`transmute` HITL; `iso.nix` comment |
| S2.1 | full-auto plan nvidia-smi/ROCm/DRM qualquer GGUF | **PARTIAL** / PREEXIST | capacity+plan pré-existente; ROCm thin |
| S2.2 | llama-server persistente stream+cancel | **PARTIAL**↑ | 0.5.0: `llama_server_posture()` honesty; SSE+cancel pré-existente; e2e daemon metal **NOT** |
| HW.1 | inventário tipado Hermes (udev/IIO/ACPI/…) | **PARTIAL** | = NX.3 thin |
| HW.2 | JARBAS cards HITL + SelfHeal firmware Nix | **PARTIAL**↑ | 0.5.0: `evaluate_self_heal` CapGate Deny-by-default + CardSpec; Slint ricos/firmware Nix **NOT** |

## Tema T / DB / F / A

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| T.1 | ModelHub 8 slots + LOO real + Trinity route | **PARTIAL** / PREEXIST | hub+route pré-existente; LOO nichos runtime residual |
| T.2 | AutoLearn Seek→fetch/treino/Factory→nsgdb | **DONE** (host) | `tick_autolearn` → `seek_before_spawn` (mesh+federated+OPAR); synth só se Seek exhaust; treino remoto residual |
| T.3 | FitPolicy/evict validate_for_slot | **PARTIAL** | `evict_coldest`/`ensure_loaded_fit` + testes hub; LOO metal/cap residual |
| DB.1 | nsgdb L2–L7 intensivo; zero JSONL | **DONE** (host default) | default `NOSD_MEMORY_BACKEND=nsgdb` → `L{2,3}/seg.bin` length-prefixed; L4 stub; JSONL só opt-in; MCP CRDT/BQ/L5–L7 = residual |
| DB.2 | OPAR Remember/Recall obrigatório todos loops | **PARTIAL**↑ | agency.tick + cognitive.tick amostrado (idle%32); AutoLearn/Sleep pré-existente; SelfHeal/mesh **NOT** |
| F.1 | Factory path A intensivo PackageHub | **DONE** (host) | Frugal wasmi path A; empty wasm → tiny identity; Seek-gated spawn |
| F.2 | SelfEvolve/SkillOpt | **PARTIAL** | `plan_skill_opt` + SelfEvolve AGENT.md bump; evolve loop pleno **NOT** |
| F.3 | Seek-Before-Generate mesh+GH/GL antes synth | **DONE** (host) | local→mesh→private→GH/GL/crates/MCP; NetAbsent honesty; `NOSD_SEEK_MOCK`; synth last; rate-limit residual |
| A.1 | Frota Agency EventDriven AGENT.md | **PARTIAL** | `tick_agency`→AGENCY_FLEET no Continuous; AGENT.md fleet pleno **NOT** |
| A.2 | Skills a quente Seek→import→wasmi | **PARTIAL** | `SkillObserver` + seek push nomes; wasmi import hot **thin** |

## Tema U / B / N / J / G

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| U.2 | k_ai Safety/Trust/Audit/SelfHeal/HWExpert nosd | **PARTIAL**↑ | 0.5.0: `evaluate_self_heal` Deny-by-default; `safety_trust` thin; HWExpert port **NOT** |
| U.3 | nos-bei budget/affect/sleep + SoulMirror | **PARTIAL** | BEI+Sleep; `tick_sleep_consolidate`→memória; SoulMirror cluster **NOT** |
| B.1 | nosd-early + BOOT SCORE + firstboot Slint | **PARTIAL**↑ | 0.5.x: early só `/var/lib/nosd`; firstboot fail-loud; ISO `NIXAIOS051`; metal journal AWAITING_HW |
| N.1 | /etc/nixos/ai-ring + generation-guard | **PARTIAL** | = NX.1 cortex-nix-ring |
| J.1 | nos-voice PipeWire full-duplex | **PARTIAL** / PREEXIST | 0.5.0: locale/piper paths Linux-first (sem FAT); EngineAbsent sem bins; duplex **thin** |
| J.2 | Generative Cards + Orb skill/hub | **PARTIAL** | `CardSpec` parse + WindowBroker; Slint generative rich widgets **NOT** |
| G.1 | Matriz/TECNOLOGIAS/addendum honesty | **DONE** | este ledger + SESSION_529 + TECNOLOGIAS N.3* |

## Tema P — Produtos (P.1–P.16)

| ID | Item | Status | Evidência / gap |
|----|------|--------|-----------------|
| P.1 | Cognitive Bridge + SOUL.md + BGE-RAG + IterationBudget | **PARTIAL** | `build_chat_prompt` SOUL+recall; BGE-RAG/IterationBudget **NOT** |
| P.2 | LatentBus [f16;256] Hermes↔Cortex↔Orb | **PARTIAL** | chat→`apply_latent`+`latent_tag`; f16[256] real **NOT** (f32 vec) |
| P.3 | AgentJev-0.6B backend S1 CapGate | **PARTIAL**↑ | gate efectivo + `guard-iso-no-s1-blob.sh` no build ISO; blob load runtime AWAITING |
| P.4 | whisper.cpp+Piper EngineAbsent off | **PARTIAL** | `VoiceEnginePosture::EngineReady` se CLI no PATH; sem bin = Absent honesto |
| P.5 | AirLLM layer-stream TooTight | **DONE** (host thin) | `run_layer_stream` + hub; llama mmap real residual |
| P.6 | Membrane + MCP + Cross-OS discover | **PARTIAL** | `MembraneGrant`; MCP bridge/discover live **NOT** |
| P.7 | Mesh crypto Noise/ML-KEM RESEARCH | **PARTIAL** | RESEARCH doc; ML-KEM **NOT** |
| P.8 | wasmi `aios_ai::infer` CapGate | **DONE** | host import + testes deny |
| P.9 | SleepCycle 5 fases consolidate nsgdb real | **PARTIAL** | Consolidate→`tick_sleep_consolidate`+SLEEP_CONSOLIDATE; nsgdb MCP real **NOT** |
| P.10 | Vision + RERANKER multimodal | **PARTIAL** | `vision_rerank` resolve keywords+EngineAbsent; blobs runtime **AWAITING** |
| P.11 | WWW/Browser suite CapGate | **PARTIAL** | `plan_www` only |
| P.12 | Marketplace assinado Ed25519 | **DONE** (host) | FNV + optional Ed25519 verify (`ed25519-compact`); trusted key ring fleet residual |
| P.12+ | Store federado índices | **DONE** (host) | `FederatedIndex` ingest Seek + RPC `store.index`; GPL→Pending; sandbox install gate |
| P.13 | FLEET_HEALTH + Hub triage + NOSD_SCORE | **PARTIAL**↑ | NosScore.summary+early_boot+s1+llama+landlock p/ HUD; triage LLM **NOT** |
| P.14 | Persona emotion/cache/heartbeats/VOICE_LATENCY | **PARTIAL** | `PersonaState` thin; cache 5-tier **NOT** |
| P.15 | BGE embeddings semantic recall | **PARTIAL** | `bge_embed` Absent+lexical fallback; semantic blob **NOT** |
| P.16 | Padrões assistd/Flowork/Anthropic Skills | **PARTIAL** | presence; docs patterns; Skills MCP **NOT** |

---

## Lição memorizada (anti-padrão)

**Marcar todo completed na UI sem cumprir o goal do plano = overclaim.**  
Próxima onda: só fechar item com (1) teste host ou (2) log runtime + linha neste ledger → DONE.

## Próxima onda sugerida (pós-0.5.0, prioridade honesty)

1. Metal stick `NIXAIOS051`: campanha [`METAL_CAMPAIGN.md`](METAL_CAMPAIGN.md) — Q5→transmute sem EACCES; cage tty7 (**AWAITING_HW** até journal anexado).  
2. Lab 2-nó fleet: [`FLEET_LAB.md`](FLEET_LAB.md) 0081 A/B + SoulMirror (M.3 residual).  
3. CRDT → sync P2P NSGDB (M.2 residual além do LWW in-process).  
4. AgentJev/BGE/Vision blobs via `blob_deliberate` LOO.  
5. llama-server e2e + AirLLM mmap real (S2.2).  
6. **S1.2 REJECT:** nunca seed ISO.  
7. Remote S2 infer no peer (M.4 hint só).  
8. Gate objectivo: `bash scripts/gate-boot-to-jarbas.sh` (stdout = ground truth IA).

## Referências

- Plano Cursor: `~/.cursor/plans/nixaios_remigração_34e29087.plan.md`  
- Playbook Boot→JARBAS: [`BOOT_TO_JARBAS.md`](BOOT_TO_JARBAS.md)  
- `docs/memory/SESSION_529.md`  
- `nixaios/docs/adr-addenda/0118-remigration-nx.md`  
- `nixaios/docs/TECNOLOGIAS_NIXAIOS.md` (N.3*)  
- IDEA #707 (NixAIOS)
