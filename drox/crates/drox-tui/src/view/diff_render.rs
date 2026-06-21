//! Rendu diff unifié (gouttière, couleurs thème, surlignage intra-ligne).

use similar::{ChangeTag, TextDiff};

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::ui::theme::ThemePalette;

const GUTTER_COL_WIDTH: usize = 4;
const MAX_WORD_DIFF_LINE_LEN: usize = 200;
const MAX_WORD_DIFF_CHANGES: usize = 8;

/// Couleurs diff dérivées du thème actif.
#[derive(Debug, Clone, Copy)]
pub struct DiffColors {
    pub add: ratatui::style::Color,
    pub remove: ratatui::style::Color,
    pub hunk: ratatui::style::Color,
    pub meta: ratatui::style::Color,
    pub ctx: ratatui::style::Color,
    pub gutter: ratatui::style::Color,
    pub status_sel: ratatui::style::Color,
}

impl DiffColors {
    #[must_use]
    pub fn from_palette(p: &ThemePalette) -> Self {
        Self {
            add: p.diff_add,
            remove: p.diff_remove,
            hunk: p.diff_hunk,
            meta: p.diff_meta,
            ctx: p.diff_ctx,
            gutter: p.diff_gutter,
            status_sel: p.diff_status,
        }
    }
}

/// Numéros de ligne source cibles (en-têtes `@@`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DiffLineMeta {
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[must_use]
pub fn build_diff_line_meta(lines: &[String]) -> Vec<DiffLineMeta> {
    let mut meta = Vec::with_capacity(lines.len());
    let mut old_line: Option<u32> = None;
    let mut new_line: Option<u32> = None;

    for line in lines {
        if let Some((o, n)) = parse_hunk_header(line) {
            old_line = Some(o);
            new_line = Some(n);
            meta.push(DiffLineMeta::default());
            continue;
        }
        if is_diff_meta_line(line) {
            meta.push(DiffLineMeta::default());
            continue;
        }
        match line.as_bytes().first() {
            Some(b' ') => {
                let entry = DiffLineMeta {
                    old_line,
                    new_line,
                };
                bump(&mut old_line);
                bump(&mut new_line);
                meta.push(entry);
            }
            Some(b'-') => {
                meta.push(DiffLineMeta {
                    old_line,
                    new_line: None,
                });
                bump(&mut old_line);
            }
            Some(b'+') => {
                meta.push(DiffLineMeta {
                    old_line: None,
                    new_line,
                });
                bump(&mut new_line);
            }
            _ => meta.push(DiffLineMeta::default()),
        }
    }
    meta
}

fn bump(n: &mut Option<u32>) {
    if let Some(v) = n {
        *v += 1;
    }
}

fn is_diff_meta_line(line: &str) -> bool {
    line.starts_with("---")
        || line.starts_with("+++")
        || line.starts_with("diff ")
        || line.starts_with("index ")
        || line.starts_with('\\')
        || (line.starts_with('—') && line.ends_with('—'))
}

/// `@@ -12,4 +12,5 @@` → `(12, 12)`.
#[must_use]
pub fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("@@") {
        return None;
    }
    let rest = trimmed.strip_prefix("@@")?.trim();
    let mut parts = rest.split_whitespace();
    let minus = parts.next()?;
    let plus = parts.next()?;
    let old = minus.strip_prefix('-')?.split(',').next()?.parse().ok()?;
    let new = plus.strip_prefix('+')?.split(',').next()?.parse().ok()?;
    Some((old, new))
}

#[must_use]
pub fn render_diff_line(
    line: &str,
    meta: DiffLineMeta,
    colors: &DiffColors,
    git_highlight: Option<(bool, bool)>,
    word_diff: Option<WordDiffPair<'_>>,
) -> Line<'static> {
    let mut spans = gutter_spans(meta, colors);

    if let Some(pair) = word_diff {
        if line.starts_with('+') {
            spans.extend(word_diff_spans(
                pair.old_text,
                pair.new_text,
                true,
                colors,
            ));
            return Line::from(spans);
        }
        if line.starts_with('-') {
            spans.extend(word_diff_spans(
                pair.old_text,
                pair.new_text,
                false,
                colors,
            ));
            return Line::from(spans);
        }
    }

    let mut style = diff_line_style(line, colors);
    if let Some((selected, clickable)) = git_highlight {
        if clickable {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if selected {
            style = style
                .fg(colors.status_sel)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
    }
    spans.push(Span::styled(line.to_string(), style));
    Line::from(spans)
}

pub struct WordDiffPair<'a> {
    pub old_text: &'a str,
    pub new_text: &'a str,
}

/// Paire `-` / `+` adjacente dans un petit hunk.
#[must_use]
pub fn word_diff_pair<'a>(
    lines: &'a [String],
    index: usize,
    hunk_bounds: (usize, usize),
) -> Option<WordDiffPair<'a>> {
    if !hunk_allows_word_diff(lines, hunk_bounds) {
        return None;
    }
    let line = &lines[index];
    if line.starts_with('+') && index > 0 {
        let prev = &lines[index - 1];
        if prev.starts_with('-') {
            let old = prev.get(1..)?;
            let new = line.get(1..)?;
            if old.len() <= MAX_WORD_DIFF_LINE_LEN && new.len() <= MAX_WORD_DIFF_LINE_LEN {
                return Some(WordDiffPair { old_text: old, new_text: new });
            }
        }
    }
    if line.starts_with('-') {
        let next = lines.get(index + 1)?;
        if next.starts_with('+') {
            let old = line.get(1..)?;
            let new = next.get(1..)?;
            if old.len() <= MAX_WORD_DIFF_LINE_LEN && new.len() <= MAX_WORD_DIFF_LINE_LEN {
                return Some(WordDiffPair { old_text: old, new_text: new });
            }
        }
    }
    None
}

#[must_use]
pub fn hunk_bounds_for_line(lines: &[String], index: usize) -> (usize, usize) {
    let mut start = 0usize;
    for (i, line) in lines.iter().enumerate().take(index + 1) {
        if parse_hunk_header(line).is_some() {
            start = i;
        }
    }
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(index + 1) {
        if parse_hunk_header(line).is_some() {
            end = i;
            break;
        }
    }
    (start, end)
}

fn hunk_allows_word_diff(lines: &[String], (start, end): (usize, usize)) -> bool {
    let mut changes = 0usize;
    for line in &lines[start..end] {
        if line.starts_with('+') || line.starts_with('-') {
            changes += 1;
            if changes > MAX_WORD_DIFF_CHANGES {
                return false;
            }
            if line.len() > MAX_WORD_DIFF_LINE_LEN + 1 {
                return false;
            }
        }
    }
    changes > 0
}

fn gutter_spans(meta: DiffLineMeta, colors: &DiffColors) -> Vec<Span<'static>> {
    let gutter = Style::default().fg(colors.gutter);
    let sep = Span::styled(" │ ", gutter);
    let old = format_line_num(meta.old_line);
    let new = format_line_num(meta.new_line);
    vec![
        Span::styled(old, gutter),
        Span::styled(" ", gutter),
        Span::styled(new, gutter),
        sep,
    ]
}

fn format_line_num(n: Option<u32>) -> String {
    match n {
        Some(v) => format!("{v:>GUTTER_COL_WIDTH$}"),
        None => " ".repeat(GUTTER_COL_WIDTH),
    }
}

fn diff_line_style(line: &str, colors: &DiffColors) -> Style {
    if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(colors.meta)
    } else if line.starts_with('+') {
        Style::default().fg(colors.add)
    } else if line.starts_with('-') {
        Style::default().fg(colors.remove)
    } else if line.starts_with('@') {
        Style::default().fg(colors.hunk)
    } else {
        Style::default().fg(colors.ctx)
    }
}

fn word_diff_spans(old: &str, new: &str, is_add: bool, colors: &DiffColors) -> Vec<Span<'static>> {
    let prefix = if is_add { "+" } else { "-" };
    let base = if is_add {
        Style::default().fg(colors.add)
    } else {
        Style::default().fg(colors.remove)
    };
    let highlight = base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let mut spans = vec![Span::styled(prefix.to_string(), base)];
    let diff = TextDiff::from_chars(old, new);
    for change in diff.iter_all_changes() {
        let text = change.value().to_string();
        if text.is_empty() {
            continue;
        }
        let style = match (change.tag(), is_add) {
            (ChangeTag::Equal, _) => base,
            (ChangeTag::Delete, false) => highlight,
            (ChangeTag::Insert, true) => highlight,
            _ => continue,
        };
        spans.push(Span::styled(text, style));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hunk_header() {
        assert_eq!(
            parse_hunk_header("@@ -12,4 +12,5 @@ fn main"),
            Some((12, 12))
        );
    }

    #[test]
    fn tracks_line_numbers_through_hunk() {
        let lines = vec![
            "@@ -1,3 +1,4 @@".into(),
            " context".into(),
            "-removed".into(),
            "+added".into(),
            " tail".into(),
        ];
        let meta = build_diff_line_meta(&lines);
        assert_eq!(meta[1].old_line, Some(1));
        assert_eq!(meta[1].new_line, Some(1));
        assert_eq!(meta[2].old_line, Some(2));
        assert_eq!(meta[2].new_line, None);
        assert_eq!(meta[3].old_line, None);
        assert_eq!(meta[3].new_line, Some(2));
        assert_eq!(meta[4].old_line, Some(3));
        assert_eq!(meta[4].new_line, Some(3));
    }

    #[test]
    fn word_diff_pair_on_adjacent_lines() {
        let lines = vec![
            "@@ -1 +1 @@".into(),
            "-foo bar".into(),
            "+foo baz".into(),
        ];
        let bounds = hunk_bounds_for_line(&lines, 2);
        assert!(word_diff_pair(&lines, 2, bounds).is_some());
    }
}
