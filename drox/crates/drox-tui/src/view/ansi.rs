//! Conversion basique séquences ANSI SGR → `ratatui::text::Line`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, Default)]
struct AnsiStyle {
    fg: Option<Color>,
    bold: bool,
    dim: bool,
    italic: bool,
}

/// Indique si le texte contient des séquences CSI SGR.
#[must_use]
pub fn contains_ansi(text: &str) -> bool {
    text.contains('\x1b')
}

/// Découpe en lignes et applique SGR ; préfixe optionnel sur la première ligne.
#[must_use]
pub fn lines_from_text(
    text: &str,
    base: Style,
    first_prefix: Option<&str>,
) -> Vec<Line<'static>> {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for (i, line) in trimmed.lines().enumerate() {
        let prefix = if i == 0 { first_prefix.unwrap_or("") } else { "" };
        let mut rendered = if contains_ansi(line) {
            line_from_ansi(line, base)
        } else {
            Line::from(Span::styled(line.to_string(), base))
        };
        if !prefix.is_empty() {
            let mut spans = vec![Span::styled(prefix.to_string(), base)];
            spans.extend(rendered.spans.drain(..));
            rendered.spans = spans;
        }
        lines.push(rendered);
    }
    lines
}

/// Convertit une ligne contenant des codes CSI `ESC [ … m` en spans stylés.
#[must_use]
pub fn line_from_ansi(text: &str, base: Style) -> Line<'static> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut style = AnsiStyle::from_base(base);
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            flush_span(&mut spans, &mut buf, style, base);
            chars.next(); // '['
            let mut seq = String::new();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    if c == 'm' {
                        apply_sgr(&mut style, &seq, base);
                    }
                    break;
                }
                seq.push(c);
            }
            continue;
        }
        buf.push(ch);
    }
    flush_span(&mut spans, &mut buf, style, base);

    if spans.is_empty() {
        Line::from(Span::styled(text.to_string(), base))
    } else {
        Line::from(spans)
    }
}

fn flush_span(spans: &mut Vec<Span<'static>>, buf: &mut String, ansi: AnsiStyle, base: Style) {
    if buf.is_empty() {
        return;
    }
    spans.push(Span::styled(
        std::mem::take(buf),
        ansi.to_ratatui(base),
    ));
}

fn apply_sgr(style: &mut AnsiStyle, seq: &str, base: Style) {
    let parts = seq.split(';').filter(|p| !p.is_empty());
    let codes: Vec<u8> = if seq.is_empty() {
        vec![0]
    } else {
        parts
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    if codes.is_empty() {
        *style = AnsiStyle::from_base(base);
        return;
    }
    let mut i = 0usize;
    while i < codes.len() {
        match codes[i] {
            0 => *style = AnsiStyle::from_base(base),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            22 => style.bold = false,
            23 => style.italic = false,
            30..=37 => style.fg = Some(ansi_fg_standard(codes[i])),
            39 => style.fg = base.fg,
            90..=97 => style.fg = Some(ansi_fg_bright(codes[i])),
            _ => {}
        }
        i += 1;
    }
}

impl AnsiStyle {
    fn from_base(base: Style) -> Self {
        Self {
            fg: base.fg,
            bold: base.add_modifier.contains(Modifier::BOLD),
            dim: base.add_modifier.contains(Modifier::DIM),
            italic: base.add_modifier.contains(Modifier::ITALIC),
        }
    }

    fn to_ratatui(self, base: Style) -> Style {
        let mut m = base.add_modifier;
        if self.bold {
            m |= Modifier::BOLD;
        }
        if self.dim {
            m |= Modifier::DIM;
        }
        if self.italic {
            m |= Modifier::ITALIC;
        }
        match self.fg.or(base.fg) {
            Some(c) => Style::default().fg(c).add_modifier(m),
            None => Style::default().add_modifier(m),
        }
    }
}

fn ansi_fg_standard(code: u8) -> Color {
    match code {
        30 => Color::Black,
        31 => Color::Red,
        32 => Color::Green,
        33 => Color::Yellow,
        34 => Color::Blue,
        35 => Color::Magenta,
        36 => Color::Cyan,
        _ => Color::White,
    }
}

fn ansi_fg_bright(code: u8) -> Color {
    match code {
        90 => Color::DarkGray,
        91 => Color::LightRed,
        92 => Color::LightGreen,
        93 => Color::LightYellow,
        94 => Color::LightBlue,
        95 => Color::LightMagenta,
        96 => Color::LightCyan,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_green_bold() {
        let line = line_from_ansi("\x1b[1;32mok\x1b[0m", Style::default());
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "ok");
    }

    #[test]
    fn plain_without_escape() {
        let line = line_from_ansi("hello", Style::default().fg(Color::Gray));
        assert_eq!(line.spans[0].content, "hello");
    }

    #[test]
    fn lines_from_text_prefixes_first_line() {
        let lines = lines_from_text(
            "\x1b[32mok\x1b[0m",
            Style::default().fg(Color::Cyan),
            Some("◂ "),
        );
        assert_eq!(lines.len(), 1);
        assert!(lines[0].to_string().contains('◂'));
        assert!(lines[0].to_string().contains("ok"));
    }

    #[test]
    fn multiline_plain() {
        let lines = lines_from_text("a\nb", Style::default(), Some("> "));
        assert_eq!(lines.len(), 2);
        assert!(lines[0].to_string().starts_with("> a"));
    }
}
