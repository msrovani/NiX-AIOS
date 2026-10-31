# Skills: confiança, qualidade e escolha pelo Hermes

**Estado:** S1, S2a, S2b e S2c implementados (0.7.0-alpha.2 in-tree): BLAKE3,
manifesto explícito, assinatura de publisher + allowlist, caps declaradas,
versão monotónica, histórico local, revogação e push assinado. O residual está
em §7.
**Objectivo:** publicar/activar skills com garantias verificáveis e deixar o
Hermes escolher, entre candidatas com o mesmo foco, a que está melhor.

---

## 1. Estado real (para não fingir)

| Propriedade | Hoje |
|---|---|
| Integridade do módulo | **BLAKE3** (64 hex) como commitment; FNV só dedup legado em `PackageStore` |
| Autoria | assinatura Ed25519 do manifesto + allowlist `NOSD_SKILL_PUBLISHERS` (S2a) |
| Manifesto | explícito em `skill/<nome>/meta`: `focus`, `version`, `caps`, `kind`, `abi`, `publisher`, `sig` |
| Capacidades | **declaradas**; execução = declaradas ∩ `NOSD_SKILL_GRANT_CAPS` (default `log`; `aios_ai.infer` opt-in) |
| Sandbox | wasmi frugal 100k fuel / 256 KiB / 4 páginas / 1 024 elementos de tabela; imports em allowlist; saída 16 KiB (ABI v2) |
| Versão / downgrade | semver monotónica por `focus`; downgrade exige grant HITL `skill.downgrade` |
| Escolha entre 2 apps do mesmo foco | `skill.explain`/`skill.select`, score 0-100 com `why` auditável |
| Histórico / revogação | `skill/<n>/stats` local; `skill/revoked/<hex>` (S2c) |

Conclusão: um módulo só entra com `FS`, hash BLAKE3 verificável, imports
allowlisted, caps validadas e (se assinado) publisher autorizado. Um módulo
malicioso pode negar serviço dentro do orçamento de fuel, mas não tem fs/net/exec.

---

## 2. Modelo de ameaças (skills)

| # | Ameaça | Hoje | Residual |
|---|---|---|---|
| T1 | Wasm adulterado em trânsito | BLAKE3 no manifesto + Noise/Ed25519 do frame | — |
| T2 | Publicador impostor | assinatura do manifesto + allowlist de publishers | rotação/revogação manual via `skill/revoked/<hex>` |
| T3 | Typosquatting (mesmo nome/foco) | `focus` + `publisher` no manifesto; `skill.explain` audita | sem pin explícito por HITL |
| T4 | Escalada de capacidades | caps declaradas ∩ `NOSD_SKILL_GRANT_CAPS`; imports allowlisted | sem quota distribuída (a quota é local ao nó) |
| T5 | Abuso de recursos | fuel/heap medidos; quota de runs+fuel por peer (`NOSD_SKILL_QUOTA_*`, reserva atómica antes de executar, persistida e assinada com a chave do nó + âncora anti-apagamento); perfil app exige opt-in | sem limite de heap por peer; quota local ao nó; sem identidade M6 fica não assinada; par janela+âncora substituído é residual |
| T6 | Downgrade para versão vulnerável | semver monotónica + HITL `skill.downgrade` | — |
| T7 | Supply chain (dependência trocada) | módulo content-addressed por BLAKE3; `active` aponta o hash | sem lock separado/`skill.lock` |
| T8 | Execução de efeitos | allowlist fechada (`aios_ai.infer`, `aios_log.write`), sem fs/net/exec | — |
| T9 | Resultado envenenado | envelope do job assinado (M7) + `provenance` no log | resultado da skill não é assinado |
| T10 | Reputação falsa | `skill/<n>/stats` **local** (sucesso/falha) | sem reputação distribuída (de propósito) |

---

## 3. Formato de pacote v2

```jsonc
// skill manifest (vive em skill/<nome>/meta; chaves reais do CRDT)
{
  "name": "summarize",
  "focus": "text.summarize",        // agrupamento para escolha
  "version": "1.2.0",               // semver, monotónica por focus
  "caps": ["log"],                  // declaração; host intersecta
  "hash": "…blake3 64 hex…",        // commitment do wasm
  "bytes": 18432,
  "kind": "wasm",                   // ou "declarative" (SKILL.md)
  "abi": 2,                         // 1 = run; 2 = run + result_ptr/len
  "profile": "frugal",              // pedido; app exige opt-in
  "provenance": "signed",           // local | peer | declarative
  "publisher": "…ed25519 hex…",
  "sig": "…base64…",                // nome|focus|version|hash
  "published_at_ms": 1761…
}
```

Regras de instalação (**fail-closed, por ordem**):

1. `wasm_blake3` do corpo **tem** de bater.
2. Assinatura do manifesto válida com `publisher`.
3. `publisher` em `trusted_publishers` (allowlist) **ou** igual ao peer que
   empurrou (`provenance=peer`) **ou** decisão HITL explícita.
4. `caps` na allowlist fechada (`aios_ai.infer`, `log`); qualquer outra →
   recusa. Em execução vale `caps ∩ NOSD_SKILL_GRANT_CAPS` (interseção, não
   erro de instalação).
5. Inspeção de imports no wasmi + execução com **zero caps**; fuel é tolerado
   (um loop pode ser intencional), qualquer outro trap recusa o módulo.
6. Versão `>=` a activa no `focus` (senão exige HITL `skill.downgrade`).
7. Só então: grava `skills/<blake3>.wasm`, `skill/<nome>/active` e o manifesto
   `skill/<nome>/meta` (com `kind`, `abi`, `profile`, `publisher`/`sig`).

Proveniência registada: `local` | `peer` (push) | `signed` (publisher).

Regras novas desde a ABI v2 (todas fail-closed):

8. **Imports**: só `aios_ai.infer` e `aios_log.write`; qualquer outro
   (incluindo `wasi_*`) = `import recusado` no install e no run.
9. **Caps em execução**: o manifesto declara, o host concede apenas a
   interseção com `NOSD_SKILL_GRANT_CAPS`. Declarar não é autorizar.
10. **Fuel medido**: `skill.run` devolve `fuel_used`/`fuel_limit`; o limite
    não é ajustável pelo módulo.
11. **Quota de execução por peer**: o peer é derivado do transporte —
    `unix:<uid>` (`SO_PEERCRED`), `tcp:token:<geração>` (o token é o peer) ou
    `jarbas-ui` (kiosk). Janela deslizante **persistida** em
    `<data-dir>/skill_quota.json` (0600, escrita atómica tmp+fsync+rename); a
    run é **reservada sob o mutex antes de ler o módulo** (dois pedidos
    concorrentes não estouram o limite) e o fuel observado é cobrado depois
    (`Fuel` trap cobra o limite do perfil; outro erro cobra 0). Estourar a
    quota devolve `ok:false` com `quota` no erro — a skill não é lida nem
    executada. Um restart do `nosd` relê a janela: o orçamento gasto **não**
    volta. Ficheiro ilegível ou de versão desconhecida = recusa global até o
    operador o remover (fail-closed, nunca um orçamento novo em silêncio);
    gravar falha = a execução seguinte é recusada.
12. **Janela assinada**: com identidade do nó (`NOSD_MESH_KEY` /
    `NOSD_MESH_REQUIRE_SIG=1`), `<data-dir>/skill_quota.json` leva Ed25519
    sobre uma forma canónica (peers ordenados) e o `node_id` do signatário.
    Ao lado fica `<data-dir>/skill_quota.anchor` com o último snapshot
    assinado. No arranque:
    - assinatura inválida, ausente, ou de **outra chave** = recusa global;
    - janela **apagada** com a âncora presente = recusa (não é instalação nova);
    - janela **recuada** (runs/fuel abaixo da âncora, ou peer que desapareceu
      da janela activa) = recusa (anti-rollback);
    - ambos apagados = instalação nova legítima (o operador assumiu o reset).
    Sem identidade (lab/release sem `require_sig`), a janela é **não assinada**
    e o arranque diz `skill quota signed=false`: a protecção contra edição
    desaparece, e isso é declarado em vez de fingido.

---

## 3.1 ABI de app Wasm (v2)

| Item | Contrato |
|---|---|
| Export obrigatório | `run(i32) -> i32` |
| Resultado (opcional) | `result_ptr() -> i32` + `result_len() -> i32`; o host lê no máximo `max_output_bytes`, valida `ptr+len` contra o linear memory, corta e marca `result_truncated` |
| Logs | import `aios_log.write(level, ptr, len) -> i32`; só com cap `log`; total limitado e truncado |
| Infer | import `aios_ai.infer() -> i32`; só com `INFER` concedido; `0` = permitido (stub de política), `-1` = negado. Não há chamada a modelo a partir do wasm |
| Imports proibidos | fs, net, exec, wasi e qualquer par fora da allowlist |
| Perfis | `frugal`: 256 KiB / 100k fuel / 4 páginas / 1 024 elementos de tabela / 16 KiB de saída (default); `app`: 1 MiB / 500k / 16 páginas / 4 096 / 64 KiB, só com `NOSD_SKILL_APP_PROFILE=1`. O manifesto pede `profile: app`; sem opt-in o host aplica `frugal` e reporta `profile_requested` |
| Cache | módulo compilado indexado por BLAKE3; `FrugalRuntime::shared()` reutiliza o `Engine` e o módulo entre chamadas |
| Fuel | `fuel_used = limite − fuel restante`; determinístico e reportado |

Exemplo mínimo (app ABI v2, sem toolchain):

```wat
(module
  (memory (export "memory") 1)
  (data (i32.const 64) "{\"ok\":true}")
  (func (export "run") (param i32) (result i32) local.get 0)
  (func (export "result_ptr") (result i32) (i32.const 64))
  (func (export "result_len") (result i32) (i32.const 11)))
```

O resultado é **dados**, não autorização: `skill.run` devolve `result` (texto
UTF-8) ou `result_b64` (bytes), além de `output` (logs), `abi` e `fuel_used`.

Em NixOS, os knobs são opções do módulo (não env solta):

```nix
services.nosd.skillGrantCaps = [ "log" ];      # NOSD_SKILL_GRANT_CAPS
services.nosd.skillAppProfile = false;         # NOSD_SKILL_APP_PROFILE
services.nosd.skillQuotaRuns = 240;            # NOSD_SKILL_QUOTA_RUNS
services.nosd.skillQuotaFuel = 5000000;        # NOSD_SKILL_QUOTA_FUEL
services.nosd.skillQuotaWindowSecs = 60;       # NOSD_SKILL_QUOTA_WINDOW_SECS
```

Os defaults são exatamente estes: `log` concedido se declarado, `infer`
negado até o operador o pôr na lista, perfil `app` desligado, 240 runs e
5 000 000 de fuel por peer a cada 60 s. `skillQuotaRuns = 0` é política válida
(nega tudo), não “sem limite”. A quota é local ao nó (não viaja no CRDT) mas
sobrevive a um restart: a janela fica em `<data-dir>/skill_quota.json`, com
âncora assinada ao lado. O tempo é de relógio de parede (epoch ms) porque um
`Instant` não atravessa o processo — um relógio que recue só alonga a janela;
um que avance expira-a mais cedo.

**Assinatura**: depende da identidade M6 (`NOSD_MESH_KEY` +
`NOSD_MESH_REQUIRE_SIG=1`, como no `iso-demo`). O módulo NixOS não liga a
identidade por omissão; sem ela o ficheiro não é assinado e o `nosd` regista
`skill quota signed=false` no arranque. Ligar a identidade é uma decisão de
postura do mesh, não deste módulo.

**Limite honesto (rollback do par)**: a âncora impede apagar/recuar a janela
com a âncora atual, mas quem escreve no data-dir pode repor **o par** janela +
âncora antigo e consistente — não há contador monotónico fora do data-dir
(mesma classe do residual da âncora do CRDT). A assinatura prova autoria e
integridade, não frescura. Na ISO de release a identidade M6 não está ligada
por omissão (só a `iso-demo` usa `NOSD_MESH_REQUIRE_SIG=1`); aí a janela sai
não assinada (`signed=false`), declarado no arranque.

---

## 4. Escolha entre candidatas do mesmo foco (Hermes)

Chave de índice: `focus/<focus>/candidates = ORSet<nome@versão>`.
`focus/<focus>/active = nome@versão` + `focus/<focus>/why = {score, breakdown}`.

**Score (0–100), determinístico e auditável:**

Implementação real (`score_meta`, `skills.rs`):

| Critério | Peso | Como mede |
|---|---|---|
| Proveniência | 25 | `signed` 25 > `peer` 15 > `local`/`declarative` 8 |
| Capacidade mínima | 15 | `15 − 3×n_caps` (menos é melhor) |
| Versão | 30 | mais recente no `focus` = 30, senão 15 |
| Tamanho | 5 | menor no `focus` = 5, senão 2 |
| Histórico local | 15 | `15 × ok/runs` com ≥5 execuções, senão 6 (neutro-baixo) |
| Eficiência de fuel | 10 | alvo = 10% do limite do perfil; `10 × alvo/média`, clamp 10; <3 runs = 5 (neutro) |

Os pesos somam 100 (clamp como cinto e tirantes). Desempate: `version`
(maior); empate total fica na ordem estável (nome). A eficiência vem de
`skill/<nome>/fuel`, telemetria **local** acumulada por `skill.run`
(`runs`, `total`, `max`, `limit`), e é explicada em
`skill.explain.why = {fuel, fuel_avg, fuel_target, fuel_runs}` — três
execuções no mesmo perfil dão média estável; uma só não premeia.

**Fluxo actual:**
1. Publicar candidato = `skill.install` com `focus`/`version`/`publisher`/`sig`
   (as regras do §3).
2. `skill.select <focus>` (ou `skill.explain`) pontua e escolhe; **o tick do
   Hermes ainda não chama isto automaticamente**.
3. A escolha é escrita no CRDT com `why` — reprodutível e auditável.
4. `skill.pin <focus> <nome@versao>` (override HITL) e fallback automático
   após 3 falhas são **desenho**, não implementados; hoje a seleção seguinte
   reescreve `focus/<focus>/active`.

**Anti-patterns:** reputação global auto-reportada; “mais downloads” como sinal;
escolher pelo tamanho apenas; actualizar automaticamente sem verificação de
assinatura.

---

## 5. Testes (negativos primeiro)

| Teste | Espera |
|---|---|
| `wasm_bytes` alterado após assinar | recusa (`hash`) |
| manifesto com sig de outra chave | recusa (`sig`) |
| publisher fora da allowlist sem HITL | recusa (`untrusted`) |
| `caps` com `fs.read` | recusa (`cap_escalation`) |
| versão inferior à activa | recusa sem HITL (`downgrade`) |
| duas candidatas do mesmo foco | escolhe a de maior score; `why` explica |
| candidata melhor publicada depois | próxima selecção troca; log regista |
| skill activa falha 3× | **desenho**: fallback para a segunda; ainda não implementado |
| módulo com loop infinito | fuel exhausted (já coberto) |
| peer excede `NOSD_SKILL_QUOTA_RUNS` | `skill_peer_quota_rejects_after_budget`: `ok:false` com `quota`, e a skill não corre |
| peer excede `NOSD_SKILL_QUOTA_FUEL` | `skill_peer_fuel_quota_rejects_after_budget`: 1.º run passa, 2.º recusado com `fuel` |
| runs concorrentes com `max_runs=1` | `quota_reserves_before_the_run_so_concurrent_calls_cannot_overshoot` |
| `NOSD_SKILL_QUOTA_RUNS=0` | `quota_zero_runs_denies_every_peer`: nega todos os peers |
| daemon reinicia com a quota gasta | `skill_quota_survives_daemon_restart` (2 runs, kill, mesmo data-dir, 3.º recusado, assinada) e `window_signed_survives_a_restart_and_is_ed25519` (0600, sig 128 hex, fuel/runs do disco) |
| `skill_quota.json` corrompido | `corrupt_quota_file_denies_instead_of_granting_a_fresh_budget`: recusa global, não orçamento novo |
| janela já expirada no disco | `expired_window_is_pruned_on_load`: peer volta a poder correr |
| editar runs/fuel da janela assinada | `editing_the_signed_window_is_refused` + e2e `tampered_signed_quota_is_refused_by_the_daemon` ("assinatura") |
| remover a assinatura / assinar com outra chave | `signed_window_without_signature_is_refused`, `window_signed_by_another_node_is_refused` |
| apagar a janela mantendo a âncora | `deleting_the_window_with_the_anchor_present_is_refused` ("apagada") |
| repor um ficheiro assinado antigo | `rollback_to_an_older_signed_window_is_refused` ("reduzida") |
| fuel médio alto no mesmo perfil | `skill_fuel_telemetry_feeds_the_score`: `why.fuel` desce e o total também |

---

## 6. Fases

- **S1 — feito:** BLAKE3 + manifesto + `focus` + versão + `skill.explain`.
- **S2/S2b — feito:** chave de publicador, assinatura do manifesto,
  allowlist, caps declaradas, versão monotónica e HITL no downgrade.
- **S2c — feito:** histórico local no score, push assinado, revogação.
- **S3 — parcial:** histórico e eficiência de fuel no score; quota de
  execução por peer (runs+fuel, janela deslizante, reserva atómica) e knobs
  `services.nosd.skillQuota*`. Falta fallback automático após 3 falhas e
  orçamento por skill (a quota é por peer, não por skill).

**Critério para chamar “seguro”:** S2 verde + testes negativos (hash, sig,
allowlist, caps, downgrade, imports, fuel, buffer OOB, quota). A frase
correcta hoje é: *“sandbox frugal com proveniência verificável e quota de
execução por peer; sem confidencialidade do módulo em repouso”*.

---

## 7. O que mudou nesta revisão (in-tree)

- `nos-wasm` passou de constantes soltas a `SandboxPolicy` + `Inspection` +
  `ExecReport`; o install recusa imports fora da allowlist **antes** de
  instanciar e o run mede fuel em vez de o estimar.
- ABI v2 permite que uma app devolva bytes (`result_ptr`/`result_len`), com
  bounds-check e corte no host; logs via `aios_log.write`, ambos limitados.
- `FrugalRuntime::shared()` + cache BLAKE3 evitam recompilar o módulo em cada
  `skill.run`; o teste `module_cache_reuses_compiled_module` prova o hit
  (`misses=1`, `hits=1`), sem alegar um ganho percentual não medido.
- `nos-factory` promove com `content_id` BLAKE3 e `ExecReport`; draft vazio é
  `placeholder` declarado, não uma app sintetizada.
- `skill.install` e `skill.push` partilham `install()`; `skill.list` junta o
  manifesto; `skill.run` devolve caps declaradas vs concedidas, fuel, logs e
  resultado.

Residual honesto: há quota por peer (runs+fuel, janela deslizante,
persistida e assinada em `skill_quota.json` + âncora), mas não é distribuída
(cada nó conta o seu). Edições, apagamento da janela e rollback de **um**
ficheiro são recusados; quem escreve no data-dir e apaga ou substitui o par
janela+âncora por um par antigo consistente ainda repõe orçamento — não há
contador monotónico externo (mesma classe do residual da âncora do CRDT).
Sem identidade M6, a janela é declaradamente não assinada. Sem assinatura do
*resultado* da skill (o envelope do job já é assinado), sem gossip de
revogação e sem medição em QEMU/metal nesta revisão.
