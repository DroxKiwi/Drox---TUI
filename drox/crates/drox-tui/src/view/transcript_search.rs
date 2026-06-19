//! Recherche dans le fil (`Ctrl+F`).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// État du mode recherche transcript.
#[derive(Debug, Clone)]
pub struct TranscriptSearchState {
    pub query: String,
    /// Indices de lignes (dans `flattened_log_lines`) contenant la requête.
    pub match_lines: Vec<usize>,
    /// Index courant dans `match_lines`.
    pub current: usize,
}

impl TranscriptSearchState {
    #[must_use]
    pub fn new(query: String, match_lines: Vec<usize>) -> Self {
        Self {
            query,
            current: 0,
            match_lines,
        }
    }

    #[must_use]
    pub fn counter_label(&self) -> String {
        if self.query.is_empty() {
            return "tapez une requête".into();
        }
        if self.match_lines.is_empty() {
            return "0/0".into();
        }
        format!("{}/{}", self.current + 1, self.match_lines.len())
    }

    #[must_use]
    pub fn current_line_index(&self) -> Option<usize> {
        self.match_lines.get(self.current).copied()
    }

    pub fn next_match(&mut self) {
        if self.match_lines.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.match_lines.len();
    }

    pub fn prev_match(&mut self) {
        if self.match_lines.is_empty() {
            return;
        }
        self.current = if self.current == 0 {
            self.match_lines.len() - 1
        } else {
            self.current - 1
        };
    }
}

/// Recalcule les lignes contenant `query` (insensible à la casse).
#[must_use]
pub fn find_matching_line_indices(lines: &[Line<'_>], query: &str) -> Vec<usize> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let q_lower = q.to_lowercase();
    lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let text = line_to_string(line);
            if text.to_lowercase().contains(&q_lower) {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

/// Scroll depuis le bas pour afficher `line_idx` (approx. centré).
#[must_use]
pub fn scroll_to_line(total_lines: usize, inner_height: usize, line_idx: usize) -> u16 {
    if total_lines == 0 {
        return 0;
    }
    let center_bias = inner_height / 2;
    let from_bottom = total_lines.saturating_sub(line_idx + center_bias);
    from_bottom.min(total_lines.saturating_sub(1)) as u16
}

/// Surligne les occurrences de `query` dans une ligne.
#[must_use]
pub fn highlight_line(line: &Line<'_>, query: &str, is_current: bool) -> Line<'static> {
    let text = line_to_string(line);
    if query.is_empty() {
        return Line::from(Span::styled(text, base_style(is_current)));
    }
    let q_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let match_style = if is_current {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Black).bg(Color::DarkGray)
    };
    let base = base_style(is_current);

    let mut spans = Vec::new();
    let mut byte = 0usize;
    while byte < text.len() {
        let Some(rel) = text_lower[byte..].find(&q_lower) else {
            spans.push(Span::styled(text[byte..].to_string(), base));
            break;
        };
        let start = byte + rel;
        let match_chars = q_lower.chars().count();
        let end = byte_end_after_n_chars(&text, start, match_chars);
        if start > byte {
            spans.push(Span::styled(text[byte..start].to_string(), base));
        }
        spans.push(Span::styled(text[start..end].to_string(), match_style));
        byte = end;
    }
    if spans.is_empty() {
        Line::from(Span::styled(text, base))
    } else {
        Line::from(spans)
    }
}

fn base_style(is_current: bool) -> Style {
    if is_current {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

/// Fin en octets après `n_chars` caractères Unicode à partir de `start_byte`.
fn byte_end_after_n_chars(text: &str, start_byte: usize, n_chars: usize) -> usize {
    if n_chars == 0 {
        return start_byte.min(text.len());
    }
    text.char_indices()
        .skip_while(|(i, _)| *i < start_byte)
        .nth(n_chars)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

fn line_to_string(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_lines_case_insensitive() {
        let lines = vec![
            Line::from("Hello world"),
            Line::from("FOO bar"),
        ];
        let hits = find_matching_line_indices(&lines, "foo");
        assert_eq!(hits, vec![1]);
    }

    #[test]
    fn scroll_centers_match() {
        let scroll = scroll_to_line(100, 10, 50);
        assert!(scroll > 0);
    }

    #[test]
    fn finds_utf8_query() {
        let lines = vec![Line::from("café résumé du cours")];
        let hits = find_matching_line_indices(&lines, "ré");
        assert_eq!(hits, vec![0]);
    }

    #[test]
    fn highlights_utf8_without_panic() {
        let line = Line::from("café résumé");
        let out = highlight_line(&line, "ré", false);
        let rendered: String = out
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(rendered.contains("ré"));
    }

    #[test]
    fn byte_end_respects_multibyte_chars() {
        let text = "café";
        assert_eq!(byte_end_after_n_chars(text, 0, 2), "ca".len());
        assert_eq!(byte_end_after_n_chars(text, 0, 4), text.len());
    }
}
