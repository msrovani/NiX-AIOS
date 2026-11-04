//! Pesquisa web — o que não está no sistema vai procurar-se fora, com limites
//! explícitos.
//!
//! Escolhas (e o que deliberadamente NÃO se faz):
//! - **Nada de scraping** de HTML de motor de busca: usa-se a *Instant Answer
//!   API* do DuckDuckGo em JSON (endpoint público, sem chave), e quando o
//!   utilizador/agente dá uma URL lê-se essa página.
//! - **https apenas** — nada de `http://`, `file://`, `data:`.
//! - Tudo tem tecto: 8 s de timeout, 256 KiB de corpo, N fontes.
//! - Offline/DNS/tls = erro tipado (`net_unavailable`), nunca uma resposta
//!   inventada. O chamador (RPC) decide o cap NET.

use serde::{Deserialize, Serialize};
use std::io::Read;

pub const MAX_BODY: u64 = 256 * 1024;
pub const TIMEOUT_SECS: u64 = 8;
pub const MAX_TEXT_CHARS: usize = 6_000;
pub const UA: &str = "nosd-cortex/0.1 (NixAIOS; +https://github.com/msrovani/NiX-AIOS)";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Research {
    pub query: String,
    pub kind: String,
    pub basis: String,
    pub sources: Vec<Source>,
    /// Texto já limpo (para o prompt), nunca HTML.
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    #[error("net_unavailable: {0}")]
    Net(String),
    #[error("bad_args: {0}")]
    BadArgs(&'static str),
    #[error("parse: {0}")]
    Parse(String),
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .user_agent(UA)
        .build()
}

/// Só `https://`; nada de esquemas locais.
pub fn is_url(q: &str) -> bool {
    let t = q.trim();
    t.starts_with("https://")
}

/// Valida uma URL de leitura. `http://` e esquemas opacos são recusados.
pub fn validate_url(url: &str) -> Result<(), ResearchError> {
    let t = url.trim();
    if !t.starts_with("https://") {
        return Err(ResearchError::BadArgs("só https:// é aceite"));
    }
    let host = t
        .trim_start_matches("https://")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if host.is_empty() || host.contains(' ') || !host.contains('.') {
        return Err(ResearchError::BadArgs("host inválido"));
    }
    if t.len() > 2048 {
        return Err(ResearchError::BadArgs("url demasiado longa"));
    }
    Ok(())
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b' ' => {
                if *b == b' ' {
                    out.push('+');
                } else {
                    out.push(*b as char);
                }
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Instant Answer → fontes ordenadas (abstracto primeiro, depois relacionados).
/// Puro: recebe o JSON, não toca na rede (testável offline).
pub fn parse_ddg(json: &str) -> Vec<Source> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<Source> = Vec::new();
    let abstract_text = v["AbstractText"].as_str().unwrap_or("").trim();
    let abstract_url = v["AbstractURL"].as_str().unwrap_or("").trim();
    if !abstract_text.is_empty() && !abstract_url.is_empty() {
        let title = v["Heading"].as_str().unwrap_or(abstract_url).trim();
        out.push(Source {
            title: truncate(title, 120),
            url: abstract_url.to_string(),
            snippet: truncate(abstract_text, 320),
        });
    }
    if let Some(topics) = v["RelatedTopics"].as_array() {
        for t in topics {
            // A API devolve grupos aninhados (`Topics`) e folhas (`Text`/`FirstURL`).
            let leaves: Vec<&serde_json::Value> = if let Some(sub) = t["Topics"].as_array() {
                sub.iter().collect()
            } else {
                vec![t]
            };
            for leaf in leaves {
                let text = leaf["Text"].as_str().unwrap_or("").trim();
                let url = leaf["FirstURL"].as_str().unwrap_or("").trim();
                if text.is_empty() || url.is_empty() {
                    continue;
                }
                if out.iter().any(|s| s.url == url) {
                    continue;
                }
                out.push(Source {
                    title: truncate(text, 120),
                    url: url.to_string(),
                    snippet: truncate(text, 240),
                });
                if out.len() >= 8 {
                    return out;
                }
            }
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= max {
        t
    } else {
        let cut: String = t.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Pergunta ao DuckDuckGo Instant Answer. Sem resultados **não** é erro: é
/// `Ok(vec![])` (o chamador distingue "nada encontrado" de "sem rede").
pub fn search(query: &str) -> Result<Vec<Source>, ResearchError> {
    let q = query.trim();
    if q.is_empty() {
        return Err(ResearchError::BadArgs("query vazia"));
    }
    if q.len() > 400 {
        return Err(ResearchError::BadArgs("query demasiado longa"));
    }
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1&t=nixaios",
        percent_encode(q)
    );
    let resp = agent()
        .get(&url)
        .call()
        .map_err(|e| ResearchError::Net(e.to_string()))?;
    if !(200..300).contains(&resp.status()) {
        return Err(ResearchError::Net(format!("status {}", resp.status())));
    }
    let mut body = String::new();
    resp.into_reader()
        .take(MAX_BODY)
        .read_to_string(&mut body)
        .map_err(|e| ResearchError::Net(e.to_string()))?;
    Ok(parse_ddg(&body))
}

/// HTML → texto legível. Sem parser de DOM: descarta `script`/`style`, remove
/// tags, colapsa espaços. Determinístico e testável.
pub fn html_to_text(html: &str, max_chars: usize) -> String {
    // 1) Remover blocos inteiros (`script`/`style`) por offsets de bytes. O
    //    `to_ascii_lowercase` preserva o comprimento em bytes, logo os índices
    //    continuam válidos na string original (UTF-8 intacto).
    let mut s = html.to_string();
    for tag in ["script", "style"] {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        loop {
            let lower = s.to_ascii_lowercase();
            let Some(a) = lower.find(&open) else { break };
            match lower[a..].find(&close) {
                Some(rel) => {
                    let end = a + rel + close.len();
                    s.replace_range(a..end, " ");
                }
                None => {
                    s.truncate(a);
                    break;
                }
            }
        }
    }
    // 2) Remover tags char-a-char (multi-byte seguro) e colapsar espaços.
    let mut out = String::with_capacity(s.len() / 2);
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let cleaned: String = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    truncate(&cleaned, max_chars)
}

/// Lê uma página https e devolve texto limpo (cap 256 KiB, 8 s).
pub fn read_url(url: &str) -> Result<String, ResearchError> {
    validate_url(url)?;
    let resp = agent()
        .get(url.trim())
        .call()
        .map_err(|e| ResearchError::Net(e.to_string()))?;
    if !(200..300).contains(&resp.status()) {
        return Err(ResearchError::Net(format!("status {}", resp.status())));
    }
    let mut buf: Vec<u8> = Vec::new();
    resp.into_reader()
        .take(MAX_BODY)
        .read_to_end(&mut buf)
        .map_err(|e| ResearchError::Net(e.to_string()))?;
    let body = String::from_utf8_lossy(&buf);
    Ok(html_to_text(&body, MAX_TEXT_CHARS))
}

/// Orquestra: URL → lê a página; pergunta → busca. Nunca entra em pânico e
/// devolve `error` em vez de sucesso inventado.
pub fn research(input: &str, max_sources: usize) -> Research {
    let q = input.trim().to_string();
    let mut out = Research {
        query: q.clone(),
        kind: if is_url(&q) { "url" } else { "search" }.into(),
        basis: "ddg_instant_answer".into(),
        sources: Vec::new(),
        text: String::new(),
        error: None,
    };
    if q.is_empty() {
        out.error = Some("bad_args: query vazia".into());
        return out;
    }
    // Um esquema explícito não-https é erro de utilização, não uma pergunta:
    // transformá-lo em busca esconderia o engano.
    if q.contains("://") && !is_url(&q) {
        out.error = Some("bad_args: só https:// é aceite".into());
        return out;
    }
    if is_url(&q) {
        out.basis = "page_read".into();
        match read_url(&q) {
            Ok(t) => {
                out.text = t;
                out.sources.push(Source {
                    title: q.clone(),
                    url: q,
                    snippet: String::new(),
                });
            }
            Err(e) => out.error = Some(e.to_string()),
        }
        return out;
    }
    match search(&q) {
        Ok(mut s) => {
            s.truncate(max_sources.max(1));
            let mut text = String::new();
            for src in &s {
                if !src.snippet.is_empty() {
                    text.push_str(&format!("- {}: {}\n", src.title, src.snippet));
                }
            }
            out.text = text;
            out.sources = s;
            if out.sources.is_empty() {
                out.basis = "no_answer".into();
            }
        }
        Err(e) => out.error = Some(e.to_string()),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANNED: &str = r#"{
        "Heading": "NixOS",
        "AbstractText": "NixOS is a Linux distribution built on Nix.",
        "AbstractURL": "https://nixos.org/",
        "RelatedTopics": [
            {"Text": "NixOS on Wikipedia", "FirstURL": "https://en.wikipedia.org/wiki/NixOS"},
            {"Name": "Group", "Topics": [
                {"Text": "Nix package manager", "FirstURL": "https://en.wikipedia.org/wiki/Nix_package_manager"},
                {"Text": "duplicado", "FirstURL": "https://en.wikipedia.org/wiki/NixOS"}
            ]}
        ]
    }"#;

    #[test]
    fn parses_abstract_and_nested_topics_with_dedup() {
        let s = parse_ddg(CANNED);
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!(s[0].url, "https://nixos.org/");
        assert_eq!(s[1].url, "https://en.wikipedia.org/wiki/NixOS");
        assert_eq!(
            s[2].url,
            "https://en.wikipedia.org/wiki/Nix_package_manager"
        );
    }

    #[test]
    fn garbage_json_is_empty_not_panic() {
        assert!(parse_ddg("nao e json").is_empty());
        assert!(parse_ddg("{}").is_empty());
    }

    #[test]
    fn only_https_is_accepted() {
        assert!(is_url("https://nixos.org/manual"));
        assert!(!is_url("http://nixos.org"));
        assert!(!is_url("nixos.org"));
        assert!(validate_url("https://nixos.org/x").is_ok());
        assert!(validate_url("http://nixos.org").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("https://localhost").is_err());
    }

    #[test]
    fn html_becomes_text_without_script_or_tags() {
        let html = "<html><head><style>body{color:red}</style></head><body>\
                    <h1>Nix &amp; NixOS</h1><script>alert('x')</script><p>usa a árvore git</p></body></html>";
        let t = html_to_text(html, 500);
        assert!(t.contains("Nix & NixOS"), "{t}");
        assert!(t.contains("usa a árvore git"), "{t}");
        assert!(!t.contains("alert"), "{t}");
        assert!(!t.contains("color:red"), "{t}");
        assert!(!t.contains('<'), "{t}");
    }

    #[test]
    fn research_without_network_reports_error_not_success() {
        // Porta 1 em loopback: falha rápido, sem depender de DNS.
        let r = research("https://127.0.0.1:1/x", 3);
        assert!(r.error.is_some(), "{r:?}");
        assert!(r.text.is_empty());
    }

    #[test]
    fn non_https_scheme_is_bad_args_not_a_search() {
        let r = research("file:///etc/passwd", 3);
        assert_eq!(r.error.as_deref(), Some("bad_args: só https:// é aceite"));
        assert!(r.sources.is_empty());
    }

    #[test]
    fn empty_query_is_bad_args() {
        let r = research("   ", 3);
        assert_eq!(r.error.as_deref(), Some("bad_args: query vazia"));
    }
}
