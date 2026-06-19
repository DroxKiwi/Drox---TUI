//! Surlignage syntaxique léger pour blocs de code (§12.6 — sans dépendance lourde).

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

const DEFAULT: Style = Style::new().fg(Color::Gray);
const KEYWORD: Style = Style::new().fg(Color::Magenta);
const STRING: Style = Style::new().fg(Color::Green);
const NUMBER: Style = Style::new().fg(Color::Cyan);
const COMMENT: Style = Style::new().fg(Color::DarkGray);
const FN_STYLE: Style = Style::new().fg(Color::Yellow);

/// Colore une ligne de bloc fenced selon le langage déclaré.
#[must_use]
pub fn highlight_code_line(line: &str, lang: &str) -> Line<'static> {
    let lang = lang.trim().to_ascii_lowercase();
    let spans = match lang.as_str() {
        "rust" | "rs" => highlight_rust(line),
        "python" | "py" => highlight_python(line),
        "javascript" | "js" | "typescript" | "ts" => highlight_js(line),
        "bash" | "sh" | "shell" | "zsh" => highlight_shell(line),
        "json" => highlight_json(line),
        _ => vec![Span::styled(line.to_string(), DEFAULT)],
    };
    Line::from(spans)
}

fn highlight_rust(line: &str) -> Vec<Span<'static>> {
    tokenize_keywords(
        line,
        &[
            "fn", "let", "mut", "pub", "use", "struct", "enum", "impl", "trait", "match", "if",
            "else", "for", "while", "loop", "return", "async", "await", "const", "static", "mod",
            "crate", "self", "Self", "true", "false", "where", "type", "move", "ref", "in",
        ],
    )
}

fn highlight_python(line: &str) -> Vec<Span<'static>> {
    tokenize_keywords(
        line,
        &[
            "def", "class", "import", "from", "return", "if", "elif", "else", "for", "while",
            "with", "as", "try", "except", "finally", "raise", "pass", "break", "continue", "True",
            "False", "None", "and", "or", "not", "in", "is", "lambda", "yield", "async", "await",
        ],
    )
}

fn highlight_js(line: &str) -> Vec<Span<'static>> {
    tokenize_keywords(
        line,
        &[
            "function", "const", "let", "var", "return", "if", "else", "for", "while", "class",
            "import", "export", "from", "async", "await", "try", "catch", "finally", "throw",
            "new", "true", "false", "null", "undefined", "typeof", "interface", "type",
        ],
    )
}

fn highlight_shell(line: &str) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return vec![Span::styled(line.to_string(), COMMENT)];
    }
    tokenize_keywords(
        line,
        &[
            "if", "then", "else", "fi", "for", "do", "done", "case", "esac", "function", "export",
            "local", "return", "echo", "cd", "exit",
        ],
    )
}

fn highlight_json(line: &str) -> Vec<Span<'static>> {
    let trimmed = line.trim();
    if trimmed.starts_with('"') && trimmed.contains(':') {
        if let Some((key, rest)) = split_json_key(trimmed) {
            return vec![
                Span::styled(key, STRING),
                Span::styled(rest.to_string(), DEFAULT),
            ];
        }
    }
    if trimmed.starts_with('"') {
        return vec![Span::styled(line.to_string(), STRING)];
    }
    if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
        return vec![Span::styled(line.to_string(), NUMBER)];
    }
    vec![Span::styled(line.to_string(), DEFAULT)]
}

fn split_json_key(s: &str) -> Option<(String, &str)> {
    let end = s[1..].find('"')? + 1;
    let key = &s[..=end];
    let rest = &s[end + 1..];
    Some((key.to_string(), rest))
}

fn tokenize_keywords(line: &str, keywords: &[&str]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut i = 0usize;
    let chars: Vec<char> = line.chars().collect();
    while i < chars.len() {
        if chars[i] == '"' || chars[i] == '\'' {
            let (s, next) = read_string(&chars, i);
            spans.push(Span::styled(s, STRING));
            i = next;
            continue;
        }
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            let rest: String = chars[i..].iter().collect();
            spans.push(Span::styled(rest, COMMENT));
            break;
        }
        if chars[i].is_ascii_alphanumeric() || chars[i] == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let style = if keywords.contains(&word.as_str()) {
                KEYWORD
            } else if word.chars().all(|c| c.is_ascii_digit() || c == '.') {
                NUMBER
            } else if start > 0 && chars[start - 1] == '.' {
                FN_STYLE
            } else {
                DEFAULT
            };
            spans.push(Span::styled(word, style));
            continue;
        }
        let ch = chars[i];
        spans.push(Span::styled(ch.to_string(), DEFAULT));
        i += 1;
    }
    if spans.is_empty() {
        spans.push(Span::styled(line.to_string(), DEFAULT));
    }
    spans
}

fn read_string(chars: &[char], start: usize) -> (String, usize) {
    let quote = chars[start];
    let mut i = start + 1;
    let mut s = String::new();
    s.push(quote);
    while i < chars.len() {
        s.push(chars[i]);
        if chars[i] == quote && chars.get(i.wrapping_sub(1)) != Some(&'\\') {
            i += 1;
            break;
        }
        i += 1;
    }
    (s, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_fn_keyword() {
        let line = highlight_code_line("pub fn main() {", "rust");
        assert!(!line.spans.is_empty());
        let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(flat.contains("fn"));
    }

    #[test]
    fn shell_comment() {
        let line = highlight_code_line("# comment", "bash");
        assert_eq!(line.spans.len(), 1);
    }
}
