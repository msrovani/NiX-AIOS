//! V6 (plano vampiro) — parser `SKILL.md` (agentskills.io) → manifesto S1.
//!
//! Origem da spec: `openjarvis/skills/{parser,loader,security}.py`.
//!
//! Regra de ouro: **declarativo**. Uma skill importada descreve intenção e
//! caps; não traz código. O piso de caps é uma **allowlist** (fail-closed):
//! `code:execute`/`shell`/`file:write`/`network:fetch` são recusados por nome,
//! e qualquer cap fora da allowlist também.

use serde::{Deserialize, Serialize};

/// Caps aceites numa skill declarativa (piso S2b, allowlist).
pub const ALLOWED_CAPS: &[&str] = &["log", "memory:read", "aios_ai.infer", "net:read"];

/// Caps explicitamente proibidas (mensagem de erro dedicada).
pub const FORBIDDEN_CAPS: &[&str] = &[
    "code:execute",
    "code_execute",
    "shell",
    "exec",
    "file:write",
    "file_read",
    "network:fetch",
    "tool:invoke",
];

/// Manifesto derivado do `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDoc {
    pub name: String,
    pub description: String,
    pub version: String,
    pub focus: String,
    pub caps: Vec<String>,
    /// Tamanho do corpo (após o frontmatter) — não guardamos o texto em claro.
    pub body_chars: usize,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn strip_quotes(v: &str) -> String {
    let v = v.trim();
    let v = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(v);
    let v = v
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(v);
    v.trim().to_string()
}

/// Parse do frontmatter YAML mínimo (chave: valor, listas `- x` ou `[a, b]`).
pub fn parse_skill_md(text: &str) -> Result<SkillDoc, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("").trim_end();
    if first.trim() != "---" {
        return Err("SKILL.md: frontmatter ausente (primeira linha tem de ser '---')".into());
    }

    let mut header: Vec<String> = Vec::new();
    let mut body_chars = 0usize;
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        header.push(line.to_string());
    }
    if !closed {
        return Err("SKILL.md: frontmatter não fechado (falta '---')".into());
    }
    for line in lines {
        body_chars += line.chars().count() + 1;
    }

    let mut name = String::new();
    let mut description = String::new();
    let mut version = String::new();
    let mut focus = String::new();
    let mut caps: Vec<String> = Vec::new();
    let mut in_caps_list = false;

    for raw in &header {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let trimmed = line.trim_start();
        if in_caps_list {
            if let Some(item) = trimmed.strip_prefix("- ") {
                caps.push(strip_quotes(item));
                continue;
            }
            in_caps_list = false;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "name" => name = strip_quotes(value),
            "description" => description = strip_quotes(value),
            "version" => version = strip_quotes(value),
            "focus" => focus = strip_quotes(value),
            "caps" => {
                let v = value.trim();
                if v.is_empty() {
                    in_caps_list = true;
                } else if v.starts_with('[') && v.ends_with(']') {
                    for item in v.trim_start_matches('[').trim_end_matches(']').split(',') {
                        let item = strip_quotes(item);
                        if !item.is_empty() {
                            caps.push(item);
                        }
                    }
                } else {
                    let item = strip_quotes(v);
                    if !item.is_empty() {
                        caps.push(item);
                    }
                }
            }
            _ => {}
        }
    }

    if !valid_name(&name) {
        return Err(format!(
            "SKILL.md: name inválido (\"{name}\"); use [a-z0-9_-] com 1..64 chars"
        ));
    }
    if description.trim().is_empty() {
        return Err("SKILL.md: description obrigatória".into());
    }
    if description.chars().count() > 500 {
        return Err("SKILL.md: description > 500 chars".into());
    }
    if version.is_empty() {
        version = "0.1.0".into();
    }
    let focus = if focus.is_empty() {
        name.clone()
    } else {
        focus
    };

    for cap in &caps {
        let c = cap.trim();
        if FORBIDDEN_CAPS.iter().any(|f| f == &c) {
            return Err(format!(
                "SKILL.md: cap proibida \"{c}\" — skills declarativas não executam código"
            ));
        }
        if !ALLOWED_CAPS.iter().any(|a| a == &c) {
            return Err(format!(
                "SKILL.md: cap fora da allowlist \"{c}\" (aceites: {})",
                ALLOWED_CAPS.join(", ")
            ));
        }
    }

    Ok(SkillDoc {
        name,
        description,
        version,
        focus,
        caps,
        body_chars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = "---\nname: arxiv-summary\ndescription: Resume papers do arXiv\nversion: 1.2.0\nfocus: research\ncaps:\n  - memory:read\n  - log\n---\n\nCorpo da skill.\n";

    #[test]
    fn parses_block_list_and_body() {
        let d = parse_skill_md(OK).expect("parse");
        assert_eq!(d.name, "arxiv-summary");
        assert_eq!(d.version, "1.2.0");
        assert_eq!(d.focus, "research");
        assert_eq!(d.caps, vec!["memory:read".to_string(), "log".to_string()]);
        assert!(d.body_chars > 0);
    }

    #[test]
    fn parses_inline_caps_and_quotes() {
        let t = "---\nname: \"triage\"\ndescription: 'Triagem de email'\ncaps: [log, aios_ai.infer]\n---\nx\n";
        let d = parse_skill_md(t).expect("parse");
        assert_eq!(d.name, "triage");
        assert_eq!(d.description, "Triagem de email");
        assert_eq!(d.caps.len(), 2);
        assert_eq!(d.version, "0.1.0");
        assert_eq!(d.focus, "triage");
    }

    #[test]
    fn rejects_code_execute_and_shell() {
        for cap in ["code:execute", "shell", "exec"] {
            let t = format!("---\nname: bad\ndescription: d\ncaps:\n  - {cap}\n---\n");
            let e = parse_skill_md(&t).unwrap_err();
            assert!(e.contains("proibida"), "cap={cap} err={e}");
        }
    }

    #[test]
    fn rejects_caps_outside_allowlist() {
        let t = "---\nname: bad\ndescription: d\ncaps: [gpu:write]\n---\n";
        let e = parse_skill_md(t).unwrap_err();
        assert!(e.contains("allowlist"), "{e}");
    }

    #[test]
    fn rejects_missing_frontmatter_and_bad_name() {
        assert!(parse_skill_md("sem frontmatter").is_err());
        assert!(parse_skill_md("---\nname: X Y\ndescription: d\n---\n").is_err());
        assert!(parse_skill_md("---\nname: ok\n---\n").is_err());
        assert!(parse_skill_md("---\nname: ok\ndescription: d\n").is_err());
    }

    #[test]
    fn rejects_long_description() {
        let long = "a".repeat(501);
        let t = format!("---\nname: ok\ndescription: {long}\n---\n");
        assert!(parse_skill_md(&t).is_err());
    }

    #[test]
    fn body_is_not_stored() {
        let d = parse_skill_md(OK).unwrap();
        let j = serde_json::to_string(&d).unwrap();
        assert!(
            !j.contains("Corpo da skill"),
            "manifesto guardou o corpo: {j}"
        );
    }
}
