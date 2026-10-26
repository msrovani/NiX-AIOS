//! Seek-before-generate (Plan F.3): local hub â mesh â private â public indexes; synthesize last.
//! Sem NET: abstain `NetAbsent` - nunca fingir que buscou.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::validate_artifact_md;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillNeed {
    pub goal: String,
    pub capabilities: Vec<String>,
    /// Frugal = adapters públicos com per_page menor; **não** salta Seek pÃºblico (F.3).
    pub frugal_budget: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateSource {
    Local,
    Mesh,
    PrivateHttp,
    GitHub,
    GitLab,
    Crates,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillCandidate {
    pub source: CandidateSource,
    pub name: String,
    pub url: String,
    pub score: f32,
    pub license: Option<String>,
    /// GPL/AGPL â Escalate HITL (não Auto install).
    pub escalate_license: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SeekSourceTried {
    Local,
    Mesh,
    PrivateHttp,
    GitHub,
    GitLab,
    Crates,
    Mcp,
}

/// Auditoria soughtâhit|miss|synth (Remember / OPAR).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SeekOutcome {
    Hit,
    Miss,
    Synth,
    NetAbsent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeekReport {
    pub sources_tried: Vec<SeekSourceTried>,
    pub candidates: Vec<SkillCandidate>,
    pub hit: Option<SkillCandidate>,
    pub synthesize_allowed: bool,
    /// Honesty when synthesis blocked (e.g. NetAbsent).
    pub synthesize_reason: Option<String>,
    pub outcome: SeekOutcome,
}

#[derive(Debug, Clone)]
pub struct SeekConfig {
    pub data_dir: PathBuf,
    pub net_disabled: bool,
    /// Catálogo mesh (SKILL_OFFER / peers) - preenchido pelo Hermes.
    pub mesh_offers: Vec<SkillCandidate>,
    pub private_allowlist_urls: Vec<String>,
    /// If set, parse instead of live GitHub HTTP (tests / offline).
    pub github_search_mock: Option<String>,
    pub hit_score_min: f32,
}

impl Default for SeekConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data"),
            net_disabled: false,
            mesh_offers: Vec::new(),
            private_allowlist_urls: Vec::new(),
            github_search_mock: None,
            hit_score_min: 0.55,
        }
    }
}

pub fn seek_skill(need: &SkillNeed, cfg: &SeekConfig) -> SeekReport {
    let mut report = SeekReport {
        sources_tried: Vec::new(),
        candidates: Vec::new(),
        hit: None,
        synthesize_allowed: false,
        synthesize_reason: None,
        outcome: SeekOutcome::Miss,
    };

    report.sources_tried.push(SeekSourceTried::Local);
    scan_local_ecosystem(&cfg.data_dir, need, &mut report.candidates);

    report.sources_tried.push(SeekSourceTried::Mesh);
    for offer in &cfg.mesh_offers {
        let mut c = offer.clone();
        c.score = score_need(need, &c.name, &need.goal);
        c.escalate_license = license_escalate(c.license.as_deref());
        if c.score >= cfg.hit_score_min * 0.5 {
            report.candidates.push(c);
        }
    }

    let net_ok = !cfg.net_disabled;
    if net_ok {
        if !cfg.private_allowlist_urls.is_empty() {
            report.sources_tried.push(SeekSourceTried::PrivateHttp);
            seek_private_http(need, cfg, &mut report.candidates);
        }

        // F.3: públicos sempre (frugal só reduz volume).
        report.sources_tried.push(SeekSourceTried::GitHub);
        seek_github(need, cfg, &mut report.candidates);

        report.sources_tried.push(SeekSourceTried::GitLab);
        seek_gitlab(need, cfg, &mut report.candidates);

        report.sources_tried.push(SeekSourceTried::Crates);
        seek_crates(need, cfg, &mut report.candidates);

        report.sources_tried.push(SeekSourceTried::Mcp);
        seek_mcp_catalog(need, &mut report.candidates);
    }

    report.candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    report
        .candidates
        .dedup_by(|a, b| a.name == b.name && a.url == b.url);

    if let Some(best) = report
        .candidates
        .iter()
        .find(|c| c.score >= cfg.hit_score_min)
        .cloned()
    {
        report.hit = Some(best);
        report.synthesize_allowed = false;
        report.outcome = SeekOutcome::Hit;
        return report;
    }

    if cfg.net_disabled {
        report.synthesize_allowed = false;
        report.synthesize_reason = Some("NetAbsent".into());
        report.outcome = SeekOutcome::NetAbsent;
        return report;
    }

    report.synthesize_allowed = true;
    report.outcome = SeekOutcome::Synth;
    report
}

/// One-liner para OPAR / remember.
pub fn seek_audit_line(report: &SeekReport) -> String {
    let hit = report
        .hit
        .as_ref()
        .map(|h| format!("{}@{}", h.name, h.url))
        .unwrap_or_else(|| "-".into());
    format!(
        "sought→{:?} sources={:?} hit={} synth={}",
        report.outcome, report.sources_tried, hit, report.synthesize_allowed
    )
}

fn license_escalate(lic: Option<&str>) -> bool {
    let Some(l) = lic else {
        return false;
    };
    let u = l.to_ascii_uppercase();
    u.contains("GPL") || u.contains("AGPL")
}

fn scan_local_ecosystem(data_dir: &Path, need: &SkillNeed, out: &mut Vec<SkillCandidate>) {
    let root = data_dir.join("ecosystem");
    if !root.is_dir() {
        return;
    }
    walk_local(&root, need, out);
}

fn walk_local(dir: &Path, need: &SkillNeed, out: &mut Vec<SkillCandidate>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in read.flatten() {
        let path = ent.path();
        if path.is_dir() {
            walk_local(&path, need, out);
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(name, "SKILL.md" | "AGENT.md" | "PLUGIN.md" | "WORKFLOW.md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let meta_ok = validate_artifact_md(&text).ok();
        let (skill_name, goal_text) = if let Some(m) = &meta_ok {
            (m.name.clone(), need.goal.clone())
        } else {
            (
                path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .unwrap_or(name)
                    .to_string(),
                need.goal.clone(),
            )
        };
        let score = score_need(need, &skill_name, &goal_text);
        if score < 0.25 {
            continue;
        }
        let url = format!("file://{}", path.display());
        out.push(SkillCandidate {
            source: CandidateSource::Local,
            name: skill_name,
            url,
            score,
            license: None,
            escalate_license: false,
        });
    }
}

fn score_need(need: &SkillNeed, name: &str, goal_hint: &str) -> f32 {
    let g = need.goal.to_lowercase();
    let n = name.to_lowercase();
    let hint = goal_hint.to_lowercase();
    if n.is_empty() {
        return 0.0;
    }
    // Exact / substring (tokens >=4) ? mesh OFFER / local SKILL.
    if n == g {
        return 0.95;
    }
    if n.len() >= 4 && g.len() >= 4 && (n.contains(&g) || g.contains(&n)) {
        return 0.9;
    }
    // Score tokens only against candidate `name` (hint ignored if == goal).
    let mut score = 0.0f32;
    let hay = if hint == g {
        n.clone()
    } else {
        format!("{n} {hint}")
    };
    for tok in need.goal.split(|c: char| !c.is_alphanumeric()) {
        let t = tok.to_lowercase();
        if t.len() >= 3 && hay.contains(&t) {
            score += 0.25;
        }
    }
    for cap in &need.capabilities {
        let c = cap.to_lowercase();
        if c.len() >= 3 && n.contains(&c) {
            score += 0.25;
        }
    }
    score.min(1.0)
}

/// MCP / Anthropic Skills catalog stub (hosts allowlist - bridge live = residual).
fn seek_mcp_catalog(need: &SkillNeed, out: &mut Vec<SkillCandidate>) {
    const CATALOG: &[(&str, &str, &str)] = &[
        (
            "anthropic-skill-md",
            "https://github.com/anthropics/skills",
            "MIT",
        ),
        (
            "clawhub-skill-pattern",
            "https://github.com/anthropics/skills",
            "MIT",
        ),
    ];
    for (name, url, lic) in CATALOG {
        let score = score_need(need, name, &need.goal);
        if score >= 0.2 {
            out.push(SkillCandidate {
                source: CandidateSource::Mcp,
                name: (*name).into(),
                url: (*url).into(),
                score,
                license: Some((*lic).into()),
                escalate_license: license_escalate(Some(lic)),
            });
        }
    }
}

#[cfg(feature = "net")]
fn seek_private_http(need: &SkillNeed, cfg: &SeekConfig, out: &mut Vec<SkillCandidate>) {
    for url in &cfg.private_allowlist_urls {
        if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1")) {
            continue;
        }
        let Ok(resp) = ureq::get(url)
            .set("User-Agent", "nos-factory/0.1 (NixAIOS seek)")
            .timeout(std::time::Duration::from_secs(3))
            .call()
        else {
            continue;
        };
        if !(200..300).contains(&resp.status()) {
            continue;
        }
        let Ok(body) = resp.into_string() else {
            continue;
        };
        // Índice JSON federado: [{name,url,license?}] ou SKILL.md
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&body) {
            for item in arr {
                let name = item
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("skill")
                    .to_string();
                let u = item
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or(url)
                    .to_string();
                let license = item
                    .get("license")
                    .and_then(|l| l.as_str())
                    .map(String::from);
                let score = score_need(need, &name, &need.goal);
                if score >= 0.25 {
                    out.push(SkillCandidate {
                        source: CandidateSource::PrivateHttp,
                        name,
                        url: u,
                        score,
                        escalate_license: license_escalate(license.as_deref()),
                        license,
                    });
                }
            }
            continue;
        }
        if let Ok(meta) = validate_artifact_md(&body) {
            let score = score_need(need, &meta.name, &need.goal);
            if score >= 0.25 {
                out.push(SkillCandidate {
                    source: CandidateSource::PrivateHttp,
                    name: meta.name,
                    url: url.clone(),
                    score,
                    license: None,
                    escalate_license: false,
                });
            }
        }
    }
}

#[cfg(not(feature = "net"))]
fn seek_private_http(_need: &SkillNeed, _cfg: &SeekConfig, _out: &mut Vec<SkillCandidate>) {}

fn github_query_url(need: &SkillNeed) -> String {
    let q: String = need
        .capabilities
        .iter()
        .chain(std::iter::once(&need.goal))
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("+");
    format!(
        "https://api.github.com/search/code?q={}+in:file+filename:SKILL.md",
        q
    )
}

/// Host allowlist - CapGate NET + deny arbitrary hosts (F.3).
fn host_allowed(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://api.github.com/")
        || lower.starts_with("https://github.com/")
        || lower.starts_with("https://gitlab.com/api/")
        || lower.starts_with("https://crates.io/api/")
}

fn per_page(need: &SkillNeed) -> u32 {
    if need.frugal_budget {
        4
    } else {
        8
    }
}

fn seek_github(need: &SkillNeed, cfg: &SeekConfig, out: &mut Vec<SkillCandidate>) {
    let query_url = github_query_url(need);
    if let Some(mock) = &cfg.github_search_mock {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(mock) {
            parse_github_items(&v, need, out);
        }
        return;
    }
    if !host_allowed(&query_url) {
        return;
    }

    #[cfg(feature = "net")]
    {
        let Ok(resp) = ureq::get(&query_url)
            .set("User-Agent", "nos-factory/0.1 (NixAIOS seek)")
            .set("Accept", "application/vnd.github+json")
            .timeout(std::time::Duration::from_secs(3))
            .call()
        else {
            tracing::warn!(target: "nosd", "seek github HTTP fail {}", query_url);
            return;
        };
        if resp.status() == 200 {
            if let Ok(body) = resp.into_string() {
                if let Ok(v) = serde_json::from_str(&body) {
                    parse_github_items(&v, need, out);
                }
            }
        }
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = (need, out);
    }
}

fn parse_github_items(v: &serde_json::Value, need: &SkillNeed, out: &mut Vec<SkillCandidate>) {
    let Some(items) = v.get("items").and_then(|i| i.as_array()) else {
        return;
    };
    let limit = per_page(need) as usize;
    for item in items.iter().take(limit) {
        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("skill")
            .to_string();
        let url = item
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let license = item
            .get("license")
            .and_then(|l| l.get("spdx_id"))
            .and_then(|s| s.as_str())
            .map(String::from);
        let escalate_license = license_escalate(license.as_deref());
        let score = score_need(need, &name, &need.goal);
        if score >= 0.25 {
            out.push(SkillCandidate {
                source: CandidateSource::GitHub,
                name,
                url,
                score,
                license,
                escalate_license,
            });
        }
    }
}

fn seek_gitlab(need: &SkillNeed, cfg: &SeekConfig, out: &mut Vec<SkillCandidate>) {
    let q = need.goal.replace(' ', "+");
    let url = format!(
        "https://gitlab.com/api/v4/projects?search={}&per_page={}",
        q,
        per_page(need)
    );
    if cfg.github_search_mock.is_some() {
        return;
    }
    if !host_allowed(&url) {
        return;
    }
    #[cfg(feature = "net")]
    {
        let Ok(resp) = ureq::get(&url)
            .set("User-Agent", "nos-factory/0.1 (NixAIOS seek)")
            .timeout(std::time::Duration::from_secs(3))
            .call()
        else {
            return;
        };
        if resp.status() != 200 {
            return;
        }
        let Ok(body) = resp.into_string() else {
            return;
        };
        let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&body) else {
            return;
        };
        for proj in arr {
            let name = proj
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("project")
                .to_string();
            let web = proj
                .get("web_url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            let score = score_need(need, &name, &need.goal);
            if score >= 0.25 {
                out.push(SkillCandidate {
                    source: CandidateSource::GitLab,
                    name,
                    url: web,
                    score,
                    license: None,
                    escalate_license: false,
                });
            }
        }
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = (need, cfg, out, url);
    }
}

fn seek_crates(need: &SkillNeed, cfg: &SeekConfig, out: &mut Vec<SkillCandidate>) {
    let q = need
        .capabilities
        .first()
        .map(|s| s.as_str())
        .unwrap_or(&need.goal);
    let url = format!(
        "https://crates.io/api/v1/crates?q={}&per_page={}",
        q,
        per_page(need)
    );
    if cfg.github_search_mock.is_some() {
        return;
    }
    if !host_allowed(&url) {
        return;
    }
    #[cfg(feature = "net")]
    {
        let Ok(resp) = ureq::get(&url)
            .set("User-Agent", "nos-factory/0.1 (NixAIOS seek)")
            .timeout(std::time::Duration::from_secs(3))
            .call()
        else {
            return;
        };
        if resp.status() != 200 {
            return;
        }
        let Ok(body) = resp.into_string() else {
            return;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
            return;
        };
        let Some(crates) = v.get("crates").and_then(|c| c.as_array()) else {
            return;
        };
        for c in crates {
            let name = c
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("crate")
                .to_string();
            let url = format!("https://crates.io/crates/{}", name);
            let license = c
                .get("license")
                .and_then(|l| l.as_str())
                .map(String::from);
            let escalate_license = license_escalate(license.as_deref());
            let score = score_need(need, &name, &need.goal);
            if score >= 0.25 {
                out.push(SkillCandidate {
                    source: CandidateSource::Crates,
                    name,
                    url,
                    score,
                    license,
                    escalate_license,
                });
            }
        }
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = (need, cfg, out, url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_md(name: &str) -> String {
        format!(
            r#"---
schema: 1
kind: skill
name: {name}
goal: echo reply
contexto: test
acionaveis: [oneshot]
---
## Contexto
x
## Goal
y
## Acionaveis
z
## Workflow
w
## Pre-Flight
p
## Success Criteria
s
## Failure Policy
f
"#
        )
    }

    #[test]
    fn miss_then_synth_allowed() {
        let tmp = TempDir::new().unwrap();
        let cfg = SeekConfig {
            data_dir: tmp.path().to_path_buf(),
            net_disabled: false,
            github_search_mock: Some(r#"{"items":[]}"#.into()),
            ..Default::default()
        };
        let need = SkillNeed {
            goal: "unique-goal-xyz".into(),
            capabilities: vec!["cap-none".into()],
            frugal_budget: false,
        };
        let r = seek_skill(&need, &cfg);
        assert!(
            r.hit.is_none(),
            "unexpected hit={:?} cands={:?}",
            r.hit,
            r.candidates
        );
        assert!(r.synthesize_allowed);
        assert_eq!(r.outcome, SeekOutcome::Synth);
    }

    #[test]
    fn net_absent_no_synth() {
        let cfg = SeekConfig {
            net_disabled: true,
            ..Default::default()
        };
        let need = SkillNeed {
            goal: "anything".into(),
            capabilities: vec![],
            frugal_budget: false,
        };
        let r = seek_skill(&need, &cfg);
        assert!(!r.synthesize_allowed);
        assert_eq!(r.outcome, SeekOutcome::NetAbsent);
        assert_eq!(r.synthesize_reason.as_deref(), Some("NetAbsent"));
    }

    #[test]
    fn local_hit_blocks_synth() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("ecosystem/skills/echo");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), sample_md("echo")).unwrap();
        let cfg = SeekConfig {
            data_dir: tmp.path().to_path_buf(),
            hit_score_min: 0.5,
            ..Default::default()
        };
        let need = SkillNeed {
            goal: "echo reply".into(),
            capabilities: vec!["echo".into()],
            frugal_budget: true,
        };
        let r = seek_skill(&need, &cfg);
        assert!(r.hit.is_some());
        assert!(!r.synthesize_allowed);
        assert_eq!(r.outcome, SeekOutcome::Hit);
    }

    #[test]
    fn github_mock_parses() {
        let mock = r#"{"items":[{"name":"mesh-skill","html_url":"https://github.com/x/y","license":{"spdx_id":"MIT"}}]}"#;
        let cfg = SeekConfig {
            github_search_mock: Some(mock.into()),
            hit_score_min: 0.3,
            ..Default::default()
        };
        let need = SkillNeed {
            goal: "mesh skill".into(),
            capabilities: vec!["mesh".into()],
            frugal_budget: false,
        };
        let r = seek_skill(&need, &cfg);
        assert!(r
            .candidates
            .iter()
            .any(|c| c.source == CandidateSource::GitHub));
    }

    #[test]
    fn frugal_still_tries_public_sources() {
        let cfg = SeekConfig {
            github_search_mock: Some(r#"{"items":[]}"#.into()),
            net_disabled: false,
            ..Default::default()
        };
        let need = SkillNeed {
            goal: "zzz".into(),
            capabilities: vec![],
            frugal_budget: true,
        };
        let r = seek_skill(&need, &cfg);
        assert!(r.sources_tried.contains(&SeekSourceTried::GitHub));
        assert!(r.sources_tried.contains(&SeekSourceTried::Mcp));
    }

    #[test]
    fn gpl_flags_escalate() {
        assert!(license_escalate(Some("GPL-3.0")));
        assert!(!license_escalate(Some("MIT")));
    }

    #[test]
    fn audit_line_mentions_outcome() {
        let r = SeekReport {
            sources_tried: vec![SeekSourceTried::Local],
            candidates: vec![],
            hit: None,
            synthesize_allowed: true,
            synthesize_reason: None,
            outcome: SeekOutcome::Synth,
        };
        assert!(seek_audit_line(&r).contains("sought?Synth") || seek_audit_line(&r).contains("Synth"));
    }
}
