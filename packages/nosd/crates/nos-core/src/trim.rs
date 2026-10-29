//! V8.3 (plano vampiro) — higienização determinística de diagnósticos.
//!
//! Origem: `smart_trim` (eco-tokens). O objectivo é o mesmo: transformar saída
//! ruidosa (stderr de build, stack trace, ANSI) num bloco pequeno e estável que
//! caiba no contexto de um modelo — **sem** depender de `regex`.
//!
//! Contrato:
//! - remove ANSI (CSI/OSC/escapes de um caractere);
//! - normaliza `\r\n`/`\r` e descarta linhas vazias e separadores (`====`);
//! - falha com >20 linhas ⇒ **5 primeiras + `…` + 15 últimas**;
//! - sucesso ⇒ 20 últimas;
//! - cap de 1000 caracteres (≈250 tokens), cortando em fronteiras de `char`.

/// ~250 tokens.
pub const MAX_DIAG_CHARS: usize = 1000;
/// Acima disto aplica-se head/tail em falha.
pub const HEAD_TAIL_THRESHOLD: usize = 20;
pub const HEAD_LINES: usize = 5;
pub const TAIL_LINES: usize = 15;

pub const EMPTY: &str = "(empty)";

/// Remove sequências ANSI (CSI, OSC e escapes de um caractere).
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                // CSI: ESC [ params intermediates final
                chars.next();
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC: ESC ] … BEL | ESC \
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // Escape com intermediários: ESC ( B, ESC # 8, …
            Some(c) if ('\u{20}'..='\u{2f}').contains(&c) => {
                chars.next();
                while let Some(c) = chars.peek().copied() {
                    if ('\u{20}'..='\u{2f}').contains(&c) {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Some(c) = chars.peek().copied() {
                    if ('\u{30}'..='\u{7e}').contains(&c) {
                        chars.next();
                    }
                }
            }
            // Escape de um caractere (ESC c, ESC 7, …)
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 4
        && t.chars()
            .all(|c| matches!(c, '-' | '=' | '_' | '*' | '~' | '.'))
}

/// Corta uma `&str` ao prefixo de `n` chars (sem panic em UTF-8).
fn prefix_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Sufixo de `n` chars.
fn suffix_chars(s: &str, n: usize) -> &str {
    let total = s.chars().count();
    if total <= n {
        return s;
    }
    let skip = total - n;
    match s.char_indices().nth(skip) {
        Some((i, _)) => &s[i..],
        None => s,
    }
}

/// Diagnóstico higienizado. `failed` decide head/tail vs cauda.
pub fn trim_diagnostic(text: &str, failed: bool) -> String {
    let clean = strip_ansi(text).replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = clean
        .split('\n')
        .filter(|l| !l.trim().is_empty())
        .filter(|l| !is_separator(l))
        .collect();
    if lines.is_empty() {
        return EMPTY.to_string();
    }
    if failed && lines.len() > HEAD_TAIL_THRESHOLD {
        let mut kept: Vec<&str> = lines.drain(..HEAD_LINES).collect();
        kept.push("…");
        kept.extend(lines.iter().skip(lines.len().saturating_sub(TAIL_LINES)));
        lines = kept;
    } else if !failed && lines.len() > HEAD_TAIL_THRESHOLD {
        lines = lines.split_off(lines.len() - HEAD_TAIL_THRESHOLD);
    }
    let joined = lines.join("\n");
    if joined.chars().count() <= MAX_DIAG_CHARS {
        return joined;
    }
    // 500 + marcador + 480 chars (fronteiras de char).
    let mut out = String::with_capacity(MAX_DIAG_CHARS + 8);
    out.push_str(prefix_chars(&joined, 500));
    out.push_str("\n…\n");
    out.push_str(suffix_chars(&joined, 480));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_csi_and_osc() {
        assert_eq!(strip_ansi("\u{1b}[31mred\u{1b}[0m ok"), "red ok");
        assert_eq!(strip_ansi("a\u{1b}]0;title\u{7}b"), "ab");
        // Escape de um caractere: ESC 7 / ESC 8 desaparecem com o seu byte.
        assert_eq!(strip_ansi("\u{1b}7x\u{1b}8"), "x");
        assert_eq!(strip_ansi("\u{1b}(Bhello"), "hello");
    }

    #[test]
    fn drops_empty_and_separator_lines() {
        // Paridade com `[-=_*~.]{4,}` do eco-tokens; "---" (3) não é separador.
        let out = trim_diagnostic("a\n\n=====\n-----\nb\n", false);
        assert_eq!(out, "a\nb");
        assert_eq!(trim_diagnostic("\n\n  \n", true), EMPTY);
    }

    #[test]
    fn failure_keeps_head_and_tail() {
        let text: String = (1..=40)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = trim_diagnostic(&text, true);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), HEAD_LINES + 1 + TAIL_LINES);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[1], "line 2");
        assert_eq!(lines[HEAD_LINES], "…");
        assert_eq!(lines.last().unwrap(), &"line 40");
        assert!(out.contains("line 26"), "tail começa em 26: {out}");
        assert!(!out.contains("line 20"), "meio descartado: {out}");
    }

    #[test]
    fn success_keeps_tail_only() {
        let text: String = (1..=30)
            .map(|i| format!("l{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = trim_diagnostic(&text, false);
        assert_eq!(out.lines().count(), HEAD_TAIL_THRESHOLD);
        assert_eq!(out.lines().next().unwrap(), "l11");
    }

    #[test]
    fn short_failure_is_kept_whole() {
        let out = trim_diagnostic("erro: x\n", true);
        assert_eq!(out, "erro: x");
    }

    #[test]
    fn caps_to_budget_on_char_boundaries() {
        // multibyte para garantir que o corte não parte um char
        let big = "é".repeat(4000);
        let out = trim_diagnostic(&big, false);
        assert!(
            out.chars().count() <= MAX_DIAG_CHARS + 3,
            "len={}",
            out.chars().count()
        );
        assert!(out.contains('\u{2026}'));
    }
}
