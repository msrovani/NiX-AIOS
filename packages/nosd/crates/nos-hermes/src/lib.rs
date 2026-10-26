//! Hermes — Observe→Plan→Act→Verify→Remember + MoE S1/S2 (manifesto NixAIOS).
//! WindowBroker: JARBAS materializa; Cortex sugere via `suggest_window_kind`.
//! OrbBroker: usabilidade + sense mic/motion; Vision MoE = conector.

mod agency;
mod autolearn;
mod blob_deliberate;
mod card;
mod self_heal;
mod cognitive_act;
mod cognitive_bridge;
mod membrane;
mod patterns_external;
mod opar_gate;
mod orb_broker;
mod skill_observer;
mod window_broker;
mod www;

pub use agency::{AgencyFleet, AgentManifest, ScheduleKind};
pub use autolearn::{plan_from_seek, plan_learn, LearnAction, LearnPlan};
pub use card::{parse_card_prompt, CardKv, CardSpec};
pub use self_heal::{evaluate_self_heal, SelfHealDecision, SelfHealRecipe};
pub use opar_gate::{is_side_effect, SIDE_EFFECT_METHODS};
pub use blob_deliberate::{
    deliberate_blob_fetch, gate_allows_fetch, iso_must_not_ship_blobs, note_persona_download,
    remember_deliberation, BlobDeliberation, BlobKind, BlobNeed,
};
pub use cognitive_act::{wrap_opar, wrap_opar_with_seek, CognitiveAct};
pub use cognitive_bridge::{build_chat_prompt, soul_path_default};
pub use membrane::{discover_peers, graft, mcp_allowed, MembraneGrant};
pub use patterns_external::{documented_patterns, CrossOsPeer, PATTERN_ANTHROPIC_SKILL_MD};
pub use orb_broker::OrbBroker;
pub use skill_observer::SkillObserver;
pub use window_broker::WindowBroker;
pub use www::{fetch_if_allowed, plan_www, WwwAction, WwwError, WwwKind};

use nos_bei::{BeiRuntime, PersonaState, SleepPhase, SLEEP_PHASE_NAMES};
use nos_bus::{EventBus, LatentBus, LatentFrame};
use nos_core::{
    CapBits, CapGate, IterationBudget, ModelSlot, OrbSense, OrbSnapshot, OrbState, UiSource,
    WindowAction, WindowKind, WindowSpec,
};
use nos_cortex::{
    CapacityProber, HotSwapController, ModelHub, RouteDecision, S2Queue, SystemLane, route_intent,
    suggest_window_kind,
};
use nos_factory::{
    plan_skill_opt, seek_audit_line, seek_skill, AppFactory, ArtifactDraft, FactoryPath,
    FederatedIndex, SeekConfig, SeekOutcome, SeekReport, SkillCandidate, SkillNeed,
};
use nos_memory::{MemoryRecord, NosMemory};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum HermesError {
    #[error("cap: {0}")]
    Cap(&'static str),
    #[error("infer: {0}")]
    Infer(String),
    #[error("memory: {0}")]
    Memory(String),
    #[error("factory: {0}")]
    Factory(String),
    #[error("ui: {0}")]
    Ui(&'static str),
}

pub struct HermesEngine {
    pub hub: ModelHub,
    pub hotswap: HotSwapController,
    pub s2: S2Queue,
    pub memory: NosMemory,
    pub bei: BeiRuntime,
    pub bus: EventBus,
    pub factory: AppFactory,
    pub windows: WindowBroker,
    pub orb: OrbBroker,
    pub latent: LatentBus,
    pub agency: Mutex<AgencyFleet>,
    pub skill_observer: SkillObserver,
    /// Catálogo mesh SKILL_OFFER (F.3).
    pub mesh_offers: Mutex<Vec<SkillCandidate>>,
    /// Índices federados privados+públicos (P.12).
    pub federated_index: Mutex<FederatedIndex>,
    /// Rotação AutoLearn (T.2) — público p/ boot wire no bin nosd.
    pub autolearn_rot: AtomicU64,
    pub cap_gen: u64,
    pub chat_budget: Mutex<IterationBudget>,
    pub persona: Mutex<PersonaState>,
    pub idle_ticks: AtomicU64,
}

impl HermesEngine {
    /// Ciclo cognitivo chat com roteamento MoE.
    pub fn chat(&self, gate: &CapGate, prompt: &str) -> Result<String, HermesError> {
        gate.allow(CapBits::INFER, self.cap_gen)
            .map_err(HermesError::Cap)?;
        {
            let mut budget = self.chat_budget.lock();
            if !budget.consume() {
                return Err(HermesError::Infer("iteration budget exhausted".into()));
            }
        }
        self.idle_ticks.store(0, Ordering::Relaxed);
        if let Some(spec) = card::parse_card_prompt(prompt) {
            let _ = self.open_generative_card(gate, self.cap_gen, &spec, UiSource::Hermes);
            let body = card::card_body_from_spec(&spec);
            let _ = self.record_opar(
                "chat",
                &format!("card_prompt len={}", prompt.len()),
                "generative_card",
                &format!("title={}", spec.title),
                "window_opened",
            );
            return Ok(format!("[card] {body}"));
        }
        let bei_budget = self
            .bei
            .tick()
            .map(|s| s.budget_remaining)
            .unwrap_or(0);
        let decision = route_intent(prompt);

        // S1 AgentJev/Laya: sem blob (NOSD_S1_BLOB / slot Agent) → ScorerAbsent + LOO blob_deliberate.
        match nos_core::s1_backend_from_env() {
            nos_core::S1Backend::AgentJev | nos_core::S1Backend::DestilledLaya => {
                let blob_ok = nos_core::s1_blob_present()
                    || self.hub.path_of(ModelSlot::Agent).is_some();
                if !blob_ok {
                    self.bus.publish(
                        "S1_SCORER_ABSENT",
                        json!({
                            "backend": format!("{:?}", nos_core::s1_backend_from_env()),
                            "effective": nos_core::s1_backend_label(),
                            "slot": "agent",
                            "loo": "blob_deliberate",
                            "note": "runtime fetch deliberated — not ISO seed",
                        }),
                    );
                }
            }
            _ => {}
        }

        // Cortex → UI hint → JARBAS window (conector; não bloqueia chat)
        if let Some(kind) = suggest_window_kind(prompt) {
            let _ = self.windows.open_from_kind(
                gate,
                self.cap_gen,
                kind,
                UiSource::Cortex,
                None,
                Some(format!("Pedido via chat: {}", prompt.chars().take(80).collect::<String>())),
            );
        }

        // Observe
        self.bus.publish(
            "HERMES_OBSERVE",
            json!({
                "prompt_len": prompt.len(),
                "slot": decision.slot.name(),
                "lane": lane_name(decision.lane),
                "reason": decision.reason,
                "bei_budget": bei_budget,
                "ui_open": self.windows.list().len(),
            }),
        );

        // Plan — LOO no slot preferido; fallback Active se niche vazio
        let slot = self.plan_slot(decision);

        // Act — S1 no HotSwap (thread chat); S2 na worker dedicada (tick não partilha)
        info!(
            target: "nosd",
            "hermes act engine={} slot={} lane={} reason={} s2_pending={}",
            self.hotswap.engine_name(),
            slot.name(),
            lane_name(decision.lane),
            decision.reason,
            self.s2.pending()
        );
        let reply = if decision.lane == SystemLane::S2Deep {
            self.bus.publish(
                "HERMES_S2_OFFLOAD",
                json!({
                    "slot": slot.name(),
                    "mode": "worker_queue",
                    "pending": self.s2.pending(),
                }),
            );
            let r = self
                .s2
                .submit_wait(prompt.to_string())
                .map_err(HermesError::Infer)?;
            if !r.ok {
                return Err(HermesError::Infer(r.text));
            }
            r.text
        } else {
            self.hotswap
                .infer(prompt)
                .map_err(|e| HermesError::Infer(e.to_string()))?
        };

        // Verify (mínimo: não vazio)
        if reply.trim().is_empty() {
            return Err(HermesError::Infer("empty reply".into()));
        }

        let observe = format!(
            "prompt_len={} slot={} lane={}",
            prompt.len(),
            slot.name(),
            lane_name(decision.lane)
        );
        let plan_note = format!("slot={} reason={}", slot.name(), decision.reason);
        let act_note = format!("reply_len={}", reply.len());
        let _ = self.record_opar("chat", &observe, &plan_note, &act_note, "non_empty_reply");

        let bei = self.bei.snapshot();
        let tick_n = self.autolearn_rot.load(Ordering::Relaxed);
        let mut frame = LatentFrame::zeros("emotion", tick_n);
        frame.v[0] = ((bei.valence as f32 + 128.0) / 255.0).clamp(0.0, 1.0);
        self.latent.publish(frame.clone());
        let _ = self.orb.apply_latent(&frame);
        let _ = self.orb.set_state(OrbState::Speaking, false);

        // Remember
        if gate.allow(CapBits::MEMORY, self.cap_gen).is_ok() {
            let key = format!("md/L2/chat/{}", Uuid::new_v4());
            self.memory
                .remember(MemoryRecord {
                    key,
                    text: format!("user: {prompt} | assistant: {reply}"),
                    scope: "session/nosd".into(),
                    entities: vec![
                        "mom/episodic".into(),
                        format!("slot/{}", slot.name()),
                        format!("lane/{}", lane_name(decision.lane)),
                    ],
                })
                .map_err(|e| HermesError::Memory(e.to_string()))?;
        }
        self.bus.publish(
            "HERMES_RESPONSE",
            json!({
                "text_len": reply.len(),
                "slot": slot.name(),
                "lane": lane_name(decision.lane),
            }),
        );
        Ok(reply)
    }

    /// Onda 4.5: S2 → fila async; S1 → infer sync + `park_done` (mesmo `chat.poll`).
    pub fn chat_submit(&self, gate: &CapGate, prompt: &str) -> Result<serde_json::Value, HermesError> {
        gate.allow(CapBits::INFER, self.cap_gen)
            .map_err(HermesError::Cap)?;
        let decision = route_intent(prompt);
        let slot = self.plan_slot(decision);
        if decision.lane == SystemLane::S2Deep {
            self.bus.publish(
                "HERMES_S2_OFFLOAD",
                json!({
                    "mode": "submit_async",
                    "slot": slot.name(),
                    "pending": self.s2.pending(),
                }),
            );
            let id = self
                .s2
                .submit(prompt.to_string())
                .map_err(HermesError::Infer)?;
            return Ok(json!({
                "id": id,
                "lane": "s2",
                "slot": slot.name(),
                "ready": false,
            }));
        }
        let reply = self
            .hotswap
            .infer(prompt)
            .map_err(|e| HermesError::Infer(e.to_string()))?;
        if reply.trim().is_empty() {
            return Err(HermesError::Infer("empty reply".into()));
        }
        let _ = self.record_opar(
            "chat.submit",
            &format!("prompt_len={}", prompt.len()),
            &format!("slot={}", slot.name()),
            &format!("reply_len={}", reply.len()),
            "s1_ready",
        );
        if gate.allow(CapBits::MEMORY, self.cap_gen).is_ok() {
            let key = format!("md/L2/chat/{}", Uuid::new_v4());
            let _ = self.memory.remember(MemoryRecord {
                key,
                text: format!("user: {prompt} | assistant: {reply}"),
                scope: "session/nosd".into(),
                entities: vec!["mom/episodic".into(), format!("slot/{}", slot.name())],
            });
        }
        let id = self.s2.park_done(reply.clone(), true);
        Ok(json!({
            "id": id,
            "lane": "s1",
            "slot": slot.name(),
            "ready": true,
            "text": reply,
        }))
    }

    pub fn chat_poll(&self, job_id: u64) -> Option<serde_json::Value> {
        self.s2.poll(job_id).map(|r| {
            json!({
                "id": r.id,
                "ok": r.ok,
                "text": r.text,
            })
        })
    }

    fn plan_slot(&self, decision: RouteDecision) -> ModelSlot {
        let cap = CapacityProber::probe();
        let try_load = |slot: ModelSlot| -> bool {
            let need = self.hub.need_mb(slot);
            self.hub.ensure_loaded_fit(
                slot,
                need,
                cap.ram_avail_mb,
                cap.vram_free_mb,
            )
        };
        let chosen = if try_load(decision.slot) {
            decision.slot
        } else if decision.slot != ModelSlot::Active && try_load(ModelSlot::Active) {
            info!(
                target: "nosd",
                "hermes slot {} miss → Active",
                decision.slot.name()
            );
            ModelSlot::Active
        } else {
            info!(target: "nosd", "hermes hub empty — engine may be cpu-echo");
            return decision.slot;
        };
        if let Some(path) = self.hub.path_of(chosen) {
            if let Ok(swapped) = self.hotswap.ensure_model_loaded(path.clone()) {
                if swapped {
                    info!(
                        target: "nosd",
                        "hermes MoE LOO hot-loaded slot={} path={}",
                        chosen.name(),
                        path.display()
                    );
                }
            }
        }
        chosen
    }

    /// Seek-before-spawn (F.3): local → mesh → private → public; remember audit.
    pub fn seek_before_spawn(&self, need: &SkillNeed) -> SeekReport {
        let private = std::env::var("NOSD_SEEK_PRIVATE_URLS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|u| u.trim().to_string())
                    .filter(|u| !u.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mock = std::env::var("NOSD_SEEK_MOCK").ok();
        let cfg = SeekConfig {
            data_dir: std::env::var("NOSD_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("data")),
            net_disabled: std::env::var("NOSD_NET_DISABLED").is_ok(),
            mesh_offers: self.mesh_offers.lock().clone(),
            private_allowlist_urls: private,
            github_search_mock: mock,
            ..Default::default()
        };
        let report = seek_skill(need, &cfg);
        {
            let mut idx = self.federated_index.lock();
            let n = idx.ingest_seek_candidates(&report.candidates);
            if n > 0 {
                self.bus.publish(
                    "FEDERATED_INDEX",
                    json!({ "ingested": n, "total": idx.len() }),
                );
            }
        }
        if let Some(hit) = report.hit.as_ref() {
            self.skill_observer.push(hit.name.clone());
        }
        let audit = seek_audit_line(&report);
        let _ = self.memory.remember(MemoryRecord {
            key: format!("md/L3/seek/{}", Uuid::new_v4()),
            text: audit.clone(),
            scope: "session/seek".into(),
            entities: vec!["mom/pattern".into(), "domain/seek".into()],
        });
        let _ = self.record_opar_seek(
            "factory.seek",
            &need.goal,
            &format!("sources={:?}", report.sources_tried),
            &audit,
            &format!("{:?}", report.outcome),
            Some(&audit),
        );
        report
    }

    pub fn push_mesh_offer(&self, cand: SkillCandidate) {
        let mut g = self.mesh_offers.lock();
        g.retain(|c| c.url != cand.url);
        g.push(cand);
        if g.len() > 64 {
            g.remove(0);
        }
    }

    /// NX.6 — OPAR persistido + EventBus `COGNITIVE_ACT`.
    pub fn record_opar(
        &self,
        domain: &str,
        observe_note: &str,
        plan_note: &str,
        act_note: &str,
        verify_note: &str,
    ) -> CognitiveAct {
        self.record_opar_seek(domain, observe_note, plan_note, act_note, verify_note, None)
    }

    /// NX.6 — OPAR com seek audit embutido no CognitiveAct.
    pub fn record_opar_seek(
        &self,
        domain: &str,
        observe_note: &str,
        plan_note: &str,
        act_note: &str,
        verify_note: &str,
        seek_audit: Option<&str>,
    ) -> CognitiveAct {
        let act = wrap_opar_with_seek(
            &self.memory,
            domain,
            observe_note,
            plan_note,
            act_note,
            verify_note,
            seek_audit,
        );
        self.bus.publish(
            "COGNITIVE_ACT",
            serde_json::to_value(&act).unwrap_or(json!({})),
        );
        act
    }

    /// A.1 — seed natives once + telemetria de frota + Remember thin (DB.2 / W6).
    pub fn tick_agency(&self) {
        let mut fleet = self.agency.lock();
        if fleet.len() == 0 {
            fleet.seed_natives();
        }
        let count = fleet.len();
        drop(fleet);
        self.bus.publish("AGENCY_FLEET", json!({ "agents": count }));
        let _ = self.record_opar(
            "agency.tick",
            &format!("agents={count}"),
            "seed_natives_if_empty",
            "publish_AGENCY_FLEET",
            "ok",
        );
    }

    /// T.2 — AutoLearn rotativo (Continuous tick).
    pub fn tick_autolearn(&self, gate: &CapGate) {
        const HINTS: &[&str] = &[
            "skill_gap",
            "wifi_skill",
            "nix_skill",
            "voice_skill",
            "mesh_skill",
        ];
        let rot = self.autolearn_rot.fetch_add(1, Ordering::Relaxed);
        let goal = std::env::var("NOSD_AUTOLEARN_GOAL").unwrap_or_else(|_| {
            HINTS[(rot as usize) % HINTS.len()].to_string()
        });
        let skill_need = SkillNeed {
            goal: goal.clone(),
            capabilities: vec![],
            frugal_budget: true,
        };
        // T.2 / F.3: Seek via Hermes (mesh + federated index + OPAR audit).
        let seek = self.seek_before_spawn(&skill_need);
        let plan = plan_from_seek(&goal, seek);
        if let Some(hit) = plan.seek.hit.as_ref() {
            self.skill_observer.push(hit.name.clone());
        }
        if gate.allow(CapBits::MEMORY, self.cap_gen).is_ok() {
            let key = format!("md/L3/autolearn/{rot}");
            let text = serde_json::to_string(&plan).unwrap_or_else(|_| plan.note.clone());
            let _ = self.memory.remember(MemoryRecord {
                key,
                text,
                scope: "session/autolearn".into(),
                entities: vec!["mom/pattern".into(), "domain/autolearn".into()],
            });
        }
        self.bus.publish(
            "AUTOLEARN_PLAN",
            serde_json::to_value(&plan).unwrap_or(json!({ "need": goal })),
        );
        let opt = plan_skill_opt(&skill_need, &plan.seek);
        let _ = self.record_opar(
            "autolearn",
            &format!("goal={goal} action={:?}", plan.action),
            &format!(
                "skill_opt={:?} score={} seek={}",
                opt.action,
                opt.candidate_score,
                seek_audit_line(&plan.seek)
            ),
            "tick_autolearn",
            "plan_recorded",
        );
    }

    fn tick_sleep_consolidate(&self, gate: &CapGate) {
        if gate.allow(CapBits::MEMORY, self.cap_gen).is_err() {
            return;
        }
        let note = format!(
            "sleep_consolidate phase={} hub_loaded={} pending_skills={}",
            self.bei.sleep_phase().name(),
            self.hub.loaded_count(),
            self.skill_observer.pending().len()
        );
        let key = format!("md/L3/sleep/consolidate/{}", Uuid::new_v4());
        let _ = self.memory.remember(MemoryRecord {
            key,
            text: note,
            scope: "session/sleep".into(),
            entities: vec!["mom/pattern".into(), "domain/sleep_cycle".into()],
        });
        self.bus.publish(
            "SLEEP_CONSOLIDATE",
            json!({ "phase": self.bei.sleep_phase().name() }),
        );
    }

    pub fn recall_context(&self, query: &str) -> Result<Vec<String>, HermesError> {
        let hits = self
            .memory
            .recall_lexical(query, 5)
            .map_err(|e| HermesError::Memory(e.to_string()))?;
        Ok(hits.into_iter().map(|h| h.text).collect())
    }

    /// Explain route without infer (telemetria / HUD).
    pub fn explain_route(&self, prompt: &str) -> RouteDecision {
        route_intent(prompt)
    }

    /// Factory A Frugal: Seek-before-generate; synth só se SeekOutcome::Synth.
    pub fn spawn_wasm_agent(
        &self,
        gate: &CapGate,
        name: &str,
        wasm: &[u8],
    ) -> Result<serde_json::Value, HermesError> {
        gate.allow(CapBits::AGENT_SPAWN, self.cap_gen)
            .map_err(HermesError::Cap)?;
        let need = SkillNeed {
            goal: name.into(),
            capabilities: vec![name.into()],
            frugal_budget: true,
        };
        let seek = self.seek_before_spawn(&need);
        if seek.outcome == SeekOutcome::Hit {
            let hit = seek.hit.as_ref().unwrap();
            return Ok(json!({
                "name": hit.name,
                "path": "seek_hit",
                "url": hit.url,
                "source": format!("{:?}", hit.source),
                "seek": seek_audit_line(&seek),
                "synthesized": false,
            }));
        }
        if seek.outcome == SeekOutcome::NetAbsent || !seek.synthesize_allowed {
            return Err(HermesError::Factory(
                seek.synthesize_reason
                    .clone()
                    .unwrap_or_else(|| "Seek incomplete — synthesize denied".into()),
            ));
        }
        let draft = ArtifactDraft {
            name: name.to_string(),
            wasm: wasm.to_vec(),
            path: FactoryPath::AWasmi,
        };
        let caps = CapBits::AGENT_SPAWN;
        let out = self
            .factory
            .validate(&draft, caps, self.cap_gen)
            .map_err(|e| HermesError::Factory(e.to_string()))?;
        let promoted = self
            .factory
            .promote(&draft, caps, self.cap_gen, true)
            .map_err(|e| HermesError::Factory(e.to_string()))?;
        self.bus.publish(
            "HERMES_AGENT_SPAWN",
            json!({ "name": name, "hash": format!("{:016x}", promoted.content_hash) }),
        );
        let _ = self.record_opar(
            "agent.spawn",
            &format!("name={name} wasm_len={}", wasm.len()),
            &seek_audit_line(&seek),
            &format!("hash={:016x} path=A_frugal", promoted.content_hash),
            "validate_ok",
        );
        Ok(json!({
            "name": promoted.name,
            "content_hash": format!("{:016x}", promoted.content_hash),
            "validate_i32": out,
            "path": "A",
            "synthesized": true,
            "seek": seek_audit_line(&seek),
        }))
    }

    /// Tick de liveness (I1) — BEI + SleepCycle phase + score bus; não bloqueia.
    pub fn cognitive_tick(&self) -> serde_json::Value {
        let gate = CapGate::new(CapBits::MEMORY, self.cap_gen);
        self.tick_agency();
        self.tick_autolearn(&gate);

        let idle = self.idle_ticks.fetch_add(1, Ordering::Relaxed);
        {
            let mut persona = self.persona.lock();
            persona.tick_heartbeat(idle);
        }

        let phase_before = self.bei.sleep_phase();
        let bei_budget = self
            .bei
            .tick()
            .map(|s| s.budget_remaining)
            .unwrap_or(0);
        let phase_after = self.bei.sleep_phase();
        if phase_after == SleepPhase::Consolidate && phase_before != SleepPhase::Consolidate {
            self.tick_sleep_consolidate(&gate);
        }

        let orb = self.orb.get();
        let persona = self.persona.lock().clone();
        let chat_budget = self.chat_budget.lock().remaining;
        let payload = json!({
            "bei_budget": bei_budget,
            "sleep_phase": self.bei.sleep_phase().name(),
            "sleep_phases": SLEEP_PHASE_NAMES,
            "persona_emotion": format!("{:?}", persona.emotion),
            "persona_heartbeat_due": persona.heartbeat_due,
            "chat_iterations_left": chat_budget,
            "hub_loaded": self.hub.loaded_count(),
            "engine": self.hotswap.engine_name(),
            "s2_pending": self.s2.pending(),
            "ui_windows": self.windows.list().len(),
            "orb_state": orb.state.as_str(),
            "orb_level": orb.sense.level,
            "latent_tag": orb.latent_tag,
            "agency_agents": self.agency.lock().len(),
            "pending_skills": self.skill_observer.pending().len(),
        });
        self.bus.publish("HERMES_TICK", payload.clone());
        // DB.2 / W6 — Remember amostrado (não a cada tick — anti-flood memory).
        if idle % 32 == 0 {
            let _ = self.record_opar(
                "cognitive.tick",
                &format!("idle={idle} phase={}", phase_after.name()),
                "agency+autolearn+bei",
                "publish_HERMES_TICK",
                "ok",
            );
        }
        payload
    }

    // --- Orb (usabilidade + lab sense) ---

    pub fn orb_get(&self) -> OrbSnapshot {
        self.orb.get()
    }

    /// Atualiza orb + EventBus quando mesh reporta peers (M.3 thin HUD).
    pub fn fleet_hud_from_mesh(
        &self,
        verdict: Option<nos_mesh::HealthVerdict>,
        peer_count: u32,
        machine_count: u32,
    ) {
        if peer_count == 0 && machine_count <= 1 {
            return;
        }
        let v = verdict
            .map(|x| x.as_str())
            .unwrap_or("unknown");
        let _ = self.orb.note_fleet(v, peer_count, machine_count);
    }

    pub fn orb_patch(&self, v: &serde_json::Value) -> OrbSnapshot {
        self.orb.patch(v)
    }

    pub fn orb_sense(&self, sense: OrbSense) -> OrbSnapshot {
        self.orb.sense(sense)
    }

    // --- Conectores UI (JARBAS) ---

    pub fn ui_open(
        &self,
        gate: &CapGate,
        kind: WindowKind,
        source: UiSource,
        title: Option<String>,
        body: Option<String>,
    ) -> Result<WindowSpec, HermesError> {
        self.windows
            .open_from_kind(gate, self.cap_gen, kind, source, title, body)
            .map_err(HermesError::Ui)
    }

    /// Wi‑Fi HITL com lista de SSIDs (scan no caller).
    pub fn ui_open_wifi(
        &self,
        gate: &CapGate,
        source: UiSource,
        ssids: Vec<String>,
    ) -> Result<WindowSpec, HermesError> {
        self.windows
            .open_wifi(gate, self.cap_gen, source, ssids)
            .map_err(HermesError::Ui)
    }

    pub fn ui_list(&self) -> Vec<WindowSpec> {
        self.windows.list()
    }

    pub fn ui_close(&self, id: u64) -> bool {
        self.windows.close(id)
    }

    pub fn ui_action(&self, gate: &CapGate, act: WindowAction) -> Result<serde_json::Value, HermesError> {
        self.windows
            .action(gate, self.cap_gen, act)
            .map_err(HermesError::Ui)
    }
}

fn lane_name(l: SystemLane) -> &'static str {
    match l {
        SystemLane::S1Local => "s1",
        SystemLane::S2Deep => "s2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nos_cortex::HotSwapController;
    use nos_memory::NosMemory;
    use tempfile::{TempDir, tempdir};

    struct TestEng {
        _dir: TempDir,
        eng: HermesEngine,
    }

    fn eng() -> TestEng {
        let dir = tempdir().unwrap();
        let hotswap = HotSwapController::new_cpu();
        let hs = hotswap.clone();
        let s2 = S2Queue::start(std::sync::Arc::new(move |p| {
            hs.infer(p).map_err(|e| e.to_string())
        }));
        let bus = EventBus::new(16);
        let eng = HermesEngine {
            hub: ModelHub::new(),
            hotswap,
            s2,
            memory: NosMemory::open(dir.path()).unwrap(),
            bei: BeiRuntime::new(1000),
            windows: WindowBroker::new(bus.clone()),
            orb: OrbBroker::new(bus.clone()),
            latent: LatentBus::new(),
            agency: Mutex::new(AgencyFleet::new()),
            skill_observer: SkillObserver::new(),
            mesh_offers: Mutex::new(Vec::new()),
            federated_index: Mutex::new(FederatedIndex::new()),
            autolearn_rot: AtomicU64::new(0),
            bus,
            factory: AppFactory::new(),
            cap_gen: 1,
            chat_budget: Mutex::new(IterationBudget::new(64)),
            persona: Mutex::new(PersonaState::default()),
            idle_ticks: AtomicU64::new(0),
        };
        TestEng { _dir: dir, eng }
    }

    #[test]
    fn chat_s1() {
        let t = eng();
        let g = CapGate::new(CapBits::FULL, 1);
        let r = t.eng.chat(&g, "oi").unwrap();
        assert!(!r.is_empty());
    }

    #[test]
    fn chat_wifi_opens_hitl_window() {
        let t = eng();
        let g = CapGate::new(CapBits::FULL, 1);
        let _ = t.eng.chat(&g, "liga o wifi da casa").unwrap();
        let wins = t.eng.ui_list();
        assert_eq!(wins.len(), 1);
        assert_eq!(wins[0].kind, WindowKind::HitlWifi);
    }

    #[test]
    fn chat_s2_via_queue() {
        let t = eng();
        let g = CapGate::new(CapBits::FULL, 1);
        let r = t
            .eng
            .chat(&g, "implemente uma arquitetura completa de migrate")
            .unwrap();
        assert!(!r.is_empty());
        assert_eq!(t.eng.s2.pending(), 0);
    }

    #[test]
    fn chat_submit_s1_ready() {
        let t = eng();
        let g = CapGate::new(CapBits::FULL, 1);
        let v = t.eng.chat_submit(&g, "oi").unwrap();
        assert_eq!(v["lane"], "s1");
        assert_eq!(v["ready"], true);
        let id = v["id"].as_u64().unwrap();
        // already consumed into park — poll may still see it once
        let _ = t.eng.chat_poll(id);
    }

    #[test]
    fn chat_submit_s2_async() {
        let t = eng();
        let g = CapGate::new(CapBits::FULL, 1);
        let v = t
            .eng
            .chat_submit(
                &g,
                "implemente uma arquitetura completa de migrate e prove que funciona",
            )
            .unwrap();
        assert_eq!(v["lane"], "s2");
        assert_eq!(v["ready"], false);
        let id = v["id"].as_u64().unwrap();
        let mut got = None;
        for _ in 0..50 {
            if let Some(r) = t.eng.chat_poll(id) {
                if r.get("pending").is_none() {
                    got = Some(r);
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let r = got.expect("s2 result");
        assert_eq!(r["ok"], true);
    }

    #[test]
    fn spawn_agent() {
        let t = eng();
        let g = CapGate::new(CapBits::FULL, 1);
        // Mock público vazio → Seek miss rápido → synth path A (sem HTTP live).
        std::env::remove_var("NOSD_NET_DISABLED");
        std::env::set_var("NOSD_SEEK_MOCK", r#"{"items":[]}"#);
        let v = t
            .eng
            .spawn_wasm_agent(&g, "xyzzy-synth-unique-9f3a", &[])
            .unwrap();
        std::env::remove_var("NOSD_SEEK_MOCK");
        assert_eq!(v["path"], "A");
        assert_eq!(v["synthesized"], true);
    }

    #[test]
    fn seek_before_spawn_net_absent() {
        let t = eng();
        std::env::set_var("NOSD_NET_DISABLED", "1");
        let r = t.eng.seek_before_spawn(&SkillNeed {
            goal: "unique-x".into(),
            capabilities: vec![],
            frugal_budget: true,
        });
        std::env::remove_var("NOSD_NET_DISABLED");
        assert!(!r.synthesize_allowed);
        assert_eq!(r.outcome, SeekOutcome::NetAbsent);
    }

    #[test]
    fn seek_hit_skips_synth() {
        let t = eng();
        std::env::set_var("NOSD_SEEK_MOCK", r#"{"items":[]}"#);
        t.eng.push_mesh_offer(SkillCandidate {
            source: nos_factory::CandidateSource::Mesh,
            name: "mesh-echo".into(),
            url: "mesh://peer/echo".into(),
            score: 0.9,
            license: Some("MIT".into()),
            escalate_license: false,
        });
        let g = CapGate::new(CapBits::FULL, 1);
        let v = t.eng.spawn_wasm_agent(&g, "mesh-echo", &[]).unwrap();
        std::env::remove_var("NOSD_SEEK_MOCK");
        assert_eq!(v["path"], "seek_hit");
        assert_eq!(v["synthesized"], false);
    }

    #[test]
    fn wrap_opar_integration() {
        let t = eng();
        let act = wrap_opar(&t.eng.memory, "hermes", "observe", "plan", "act", "verify");
        assert!(act.remembered);
    }
}
