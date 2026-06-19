//! Liens URL — surlignage + OSC 8 (terminaux compatibles).

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

const URL_PREFIXES: &[&str] = &["http://", "https://", "ftp://"];

/// Découpe un texte en spans avec URLs auto-liées (OSC 8 + souligné).
#[must_use]
pub fn spans_with_autolinks(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let Some((idx, url_start)) = find_url_start(rest) else {
            if !rest.is_empty() {
                spans.push(Span::styled(rest.to_string(), base));
            }
            break;
        };
        if idx > 0 {
            spans.push(Span::styled(rest[..idx].to_string(), base));
        }
        let url_body = &rest[idx + url_start.len()..];
        let url_len = url_end_byte_len(url_body);
        let url = format!("{}{}", &rest[idx..idx + url_start.len()], &url_body[..url_len]);
        let label = url.clone();
        let link_style = base
            .fg(ratatui::style::Color::LightBlue)
            .add_modifier(Modifier::UNDERLINED);
        spans.push(Span::styled(osc8_link(&url, &label), link_style));
        rest = &rest[idx + url_start.len() + url_len..];
    }
    if spans.is_empty() && !text.is_empty() {
        spans.push(Span::styled(text.to_string(), base));
    }
    spans
}

fn find_url_start(s: &str) -> Option<(usize, &'static str)> {
    let mut best: Option<(usize, &'static str)> = None;
    for prefix in URL_PREFIXES {
        if let Some(idx) = s.find(prefix) {
            if best.map(|(i, _)| idx < i).unwrap_or(true) {
                best = Some((idx, *prefix));
            }
        }
    }
    best
}

fn url_end_byte_len(s: &str) -> usize {
    s.char_indices()
        .find(|(_, c)| c.is_whitespace() || matches!(c, ')' | ']' | '>' | '"' | '\''))
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Enveloppe OSC 8 — clic ouvre l'URL dans les terminaux compatibles.
#[must_use]
pub fn osc8_link(url: &str, label: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn finds_https_in_text() {
        let spans = spans_with_autolinks(
            "voir https://example.com/page ok",
            Style::default().fg(Color::White),
        );
        assert!(spans.len() >= 3);
        assert!(spans.iter().any(|s| s.content.contains("example.com")));
    }

    #[test]
    fn osc8_wraps_url() {
        let s = osc8_link("https://drox.dev", "https://drox.dev");
        assert!(s.contains("\x1b]8;;"));
    }
}
