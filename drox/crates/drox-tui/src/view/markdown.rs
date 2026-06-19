//! Rendu markdown minimal pour le fil assistant (pulldown-cmark → ratatui).

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::syntax;

const BASE: Style = Style::new().fg(Color::Cyan);
const MUTED: Style = Style::new().fg(Color::DarkGray);
const BOLD: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
const ITALIC: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::ITALIC);
const CODE: Style = Style::new().fg(Color::Yellow);
const CODE_BLOCK: Style = Style::new().fg(Color::Gray);
const HEADING: Style = Style::new().fg(Color::LightCyan).add_modifier(Modifier::BOLD);
const LINK: Style = Style::new().fg(Color::LightBlue).add_modifier(Modifier::UNDERLINED);

/// Convertit un bloc assistant en lignes ratatui stylées.
#[must_use]
pub fn assistant_lines(text: &str) -> Vec<Line<'static>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if super::ansi::contains_ansi(trimmed) {
        return super::ansi::lines_from_text(trimmed, BASE, Some("◂ "));
    }
    let mut lines = render_markdown(trimmed);
    if lines.is_empty() {
        return vec![Line::from(Span::styled(
            format!("◂ {trimmed}"),
            BASE,
        ))];
    }
    prepend_marker(&mut lines, "◂ ");
    lines
}

fn prepend_marker(lines: &mut [Line<'static>], marker: &str) {
    if let Some(first) = lines.first_mut() {
        let mut spans = vec![Span::styled(marker.to_string(), BASE)];
        spans.extend(first.spans.drain(..));
        first.spans = spans;
    }
}

fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(text, opts);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![BASE];
    let mut in_code_block = false;
    let mut code_block_buf = String::new();
    let mut code_block_lang = String::new();
    let mut list_depth = 0usize;

    let flush = |lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>| {
        if spans.is_empty() {
            return;
        }
        lines.push(Line::from(std::mem::take(spans)));
    };

    for event in parser {
        match event {
            Event::Text(t) => {
                if in_code_block {
                    code_block_buf.push_str(&t);
                } else {
                    let style = *style_stack.last().unwrap_or(&BASE);
                    spans.extend(super::link::spans_with_autolinks(&t, style));
                }
            }
            Event::Code(c) => {
                spans.push(Span::styled(c.to_string(), CODE));
            }
            Event::SoftBreak | Event::HardBreak => {
                flush(&mut lines, &mut spans);
            }
            Event::Start(tag) => match tag {
                Tag::Strong => style_stack.push(BOLD),
                Tag::Emphasis => style_stack.push(ITALIC),
                Tag::Strikethrough => style_stack.push(
                    Style::new()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
                Tag::Link { dest_url, .. } => {
                    style_stack.push(LINK);
                    spans.push(Span::styled(
                        format!("[{dest_url}]"),
                        Style::new().fg(Color::DarkGray),
                    ));
                }
                Tag::Heading { level, .. } => {
                    flush(&mut lines, &mut spans);
                    let marks = match level {
                        pulldown_cmark::HeadingLevel::H1 => "# ",
                        pulldown_cmark::HeadingLevel::H2 => "## ",
                        pulldown_cmark::HeadingLevel::H3 => "### ",
                        _ => "#### ",
                    };
                    spans.push(Span::styled(marks.to_string(), HEADING));
                    style_stack.push(HEADING);
                }
                Tag::List(_) => list_depth += 1,
                Tag::Item => {
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    spans.push(Span::styled(format!("{indent}• "), MUTED));
                }
                Tag::CodeBlock(kind) => {
                    flush(&mut lines, &mut spans);
                    in_code_block = true;
                    code_block_buf.clear();
                    code_block_lang.clear();
                    if let CodeBlockKind::Fenced(lang) = kind {
                        code_block_lang = lang.to_string();
                        if !lang.is_empty() {
                            lines.push(Line::from(Span::styled(
                                format!("```{lang}"),
                                MUTED,
                            )));
                        }
                    }
                }
                Tag::Paragraph => {}
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                    style_stack.pop();
                }
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    flush(&mut lines, &mut spans);
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    let lang = std::mem::take(&mut code_block_lang);
                    for line in code_block_buf.lines() {
                        let mut highlighted = syntax::highlight_code_line(line, &lang);
                        if !highlighted.spans.is_empty() {
                            let first = &mut highlighted.spans[0];
                            first.content = format!("  {}", first.content).into();
                        }
                        lines.push(highlighted);
                    }
                    if code_block_buf.is_empty() {
                        lines.push(Line::from(Span::styled("  ", CODE_BLOCK)));
                    }
                    code_block_buf.clear();
                }
                TagEnd::Paragraph => flush(&mut lines, &mut spans),
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    flush(&mut lines, &mut spans);
                }
                TagEnd::Item => flush(&mut lines, &mut spans),
                _ => {}
            },
            Event::Rule => {
                flush(&mut lines, &mut spans);
                lines.push(Line::from(Span::styled(
                    "────────────────",
                    MUTED,
                )));
            }
            _ => {}
        }
    }
    flush(&mut lines, &mut spans);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_inline_code() {
        let lines = render_markdown("use `foo` here");
        let flat: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
            .collect();
        assert!(flat.contains("foo"));
    }

    #[test]
    fn renders_rust_codeblock() {
        let lines = render_markdown("```rust\nfn main() {}\n```");
        let flat: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
            .collect();
        assert!(flat.contains("fn"));
    }

    #[test]
    fn assistant_ansi_bypasses_markdown() {
        let lines = assistant_lines("\x1b[1;32mok\x1b[0m");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].to_string().contains("◂"));
        assert!(lines[0].to_string().contains("ok"));
    }
}
