//! Texto de saída de modelos: separar raciocínio da resposta.
//!
//! Modelos "thinking" (Qwen3.x) devolvem o raciocínio em `<think>…</think>`.
//! Num chat isso não é resposta: ou o servidor já o separa
//! (`reasoning_content`), ou tem de ser removido aqui. Nunca mostrar tags
//! cruas ao utilizador — foi exactamente o que aconteceu no metal
//! (`JARBAS: <think>` e mais nada).
//!
//! Tudo puro: sem I/O, testável.

/// Marcador do turno do utilizador em `build_chat_prompt` (nos-hermes).
pub const USER_MARK: &str = "## User";

/// Remove blocos `<think>…</think>` (e `<thinking>…</thinking>`), incluindo um
/// bloco aberto que nunca fecha (o modelo esgotou `n_predict` a pensar).
pub fn strip_think(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some((start, tag_len)) = find_open(rest) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + tag_len..];
        match find_close(after_open) {
            Some((i, len)) => rest = &after_open[i + len..],
            // Aberto até ao fim: descartar o raciocínio incompleto.
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// `(início, comprimento)` da primeira tag de abertura, minúsculas ou não.
fn find_open(s: &str) -> Option<(usize, usize)> {
    let lower = s.to_ascii_lowercase();
    for tag in ["<think>", "<thinking>"] {
        if let Some(i) = lower.find(tag) {
            return Some((i, tag.len()));
        }
    }
    None
}

fn find_close(s: &str) -> Option<(usize, usize)> {
    let lower = s.to_ascii_lowercase();
    for tag in ["</think>", "</thinking>"] {
        if let Some(i) = lower.find(tag) {
            return Some((i, tag.len()));
        }
    }
    None
}

/// Separa o prompt composto (`## Persona` + `## Memory recall` + `## User`)
/// em (system, user) para o template de chat do modelo.
///
/// Sem marcador: tudo vai como `user` (nunca se perde a pergunta).
pub fn split_chat_prompt(prompt: &str) -> (String, String) {
    let lower = prompt.to_ascii_lowercase();
    let mark = USER_MARK.to_ascii_lowercase();
    match lower.find(&mark) {
        Some(i) => {
            let system = prompt[..i].trim().to_string();
            let user = prompt[i + USER_MARK.len()..].trim().to_string();
            (system, user)
        }
        None => (String::new(), prompt.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_complete_think_block() {
        assert_eq!(strip_think("<think>hmm</think>Olá!"), "Olá!");
        assert_eq!(strip_think("<THINK>a\nb</THINK>\n  resposta  "), "resposta");
    }

    #[test]
    fn strips_unclosed_think_block() {
        // Caso real do metal: esgotou n_predict a pensar -> sem resposta.
        assert_eq!(strip_think("<think>a pensar para sempre"), "");
    }

    #[test]
    fn keeps_plain_text() {
        assert_eq!(strip_think("  só texto  "), "só texto");
    }

    #[test]
    fn think_in_the_middle() {
        assert_eq!(strip_think("a<think>x</think>b"), "ab");
    }

    #[test]
    fn split_prompt_uses_user_mark() {
        let (s, u) = split_chat_prompt("## Persona (SOUL)\nJARBAS\n## User\nfale pt-br\n");
        assert!(s.starts_with("## Persona (SOUL)"));
        assert_eq!(u, "fale pt-br");
    }

    #[test]
    fn split_prompt_without_mark_keeps_everything_as_user() {
        let (s, u) = split_chat_prompt("fale pt-br");
        assert!(s.is_empty());
        assert_eq!(u, "fale pt-br");
    }
}
