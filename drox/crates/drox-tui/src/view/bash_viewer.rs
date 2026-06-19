//! Contenu bash pour le viewer scrollable (Sprint 4.1).

use drox_types::ToolUseId;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use serde_json::Value;

use super::ansi;
use super::syntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BashStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub struct BashOutputLine {
    pub stream: BashStream,
    pub text: String,
}

/// Viewer scrollable pour sortie `bash` ou mode `!`.
#[derive(Debug, Clone)]
pub struct BashViewerState {
    pub tool_id: ToolUseId,
    pub command: String,
    pub user_mode: bool,
    pub exit_code: Option<i64>,
    pub timed_out: bool,
    pub is_error: bool,
    pub output_truncated: bool,
    pub lines: Vec<BashOutputLine>,
    pub scroll_top: usize,
}

impl BashViewerState {
    #[must_use]
    pub fn from_output(
        id: ToolUseId,
        output: &Value,
        is_error: bool,
        user_mode: bool,
        command: Option<String>,
    ) -> Option<Self> {
        let parsed = parse_bash_output(output)?;
        let command = command
            .or_else(|| output.get("command").and_then(Value::as_str).map(str::to_string))
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| "(commande inconnue)".into());
        Some(Self {
            tool_id: id,
            command,
            user_mode,
            exit_code: parsed.exit_code,
            timed_out: parsed.timed_out,
            is_error,
            output_truncated: parsed.output_truncated,
            lines: parsed.lines,
            scroll_top: 0,
        })
    }

    pub fn scroll_page(&mut self, delta_lines: usize, visible: usize) {
        if delta_lines == 0 || self.lines.is_empty() {
            return;
        }
        let max_top = self.lines.len().saturating_sub(visible.max(1));
        self.scroll_top = self.scroll_top.saturating_sub(delta_lines).min(max_top);
    }

    pub fn scroll_page_down(&mut self, delta_lines: usize, visible: usize) {
        if delta_lines == 0 || self.lines.is_empty() {
            return;
        }
        let max_top = self.lines.len().saturating_sub(visible.max(1));
        self.scroll_top = (self.scroll_top + delta_lines).min(max_top);
    }

    #[must_use]
    pub fn render_lines(&self, visible: usize) -> Vec<Line<'static>> {
        let end = (self.scroll_top + visible).min(self.lines.len());
        self.lines[self.scroll_top..end]
            .iter()
            .map(|line| render_bash_line(line))
            .collect()
    }
}

struct ParsedBashOutput {
    exit_code: Option<i64>,
    timed_out: bool,
    output_truncated: bool,
    lines: Vec<BashOutputLine>,
}

fn render_bash_line(line: &BashOutputLine) -> Line<'static> {
    let base = match line.stream {
        BashStream::Stdout => Style::default().fg(Color::Gray),
        BashStream::Stderr => Style::default().fg(Color::Red),
    };
    let prefix = match line.stream {
        BashStream::Stdout => "  ",
        BashStream::Stderr => "⚠ ",
    };
    if line.text.contains('\x1b') {
        let mut spans = vec![ratatui::text::Span::styled(prefix.to_string(), base)];
        let mut ansi_line = ansi::line_from_ansi(&line.text, base);
        spans.append(&mut ansi_line.spans);
        return Line::from(spans);
    }
    let highlighted = syntax::highlight_code_line(&line.text, "shell");
    let mut spans = vec![ratatui::text::Span::styled(prefix.to_string(), base)];
    spans.extend(highlighted.spans.into_iter().map(|s| {
        let style = if line.stream == BashStream::Stderr {
            s.style.fg(Color::Red)
        } else {
            s.style
        };
        ratatui::text::Span::styled(s.content, style)
    }));
    Line::from(spans)
}

#[must_use]
fn parse_bash_output(output: &Value) -> Option<ParsedBashOutput> {
    let stdout = output.get("stdout").and_then(Value::as_str).unwrap_or("");
    let stderr = output.get("stderr").and_then(Value::as_str).unwrap_or("");
    let exit_code = output.get("exit_code").and_then(Value::as_i64);
    let timed_out = output
        .get("timed_out")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let stdout_trunc = output
        .get("stdout_truncated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let stderr_trunc = output
        .get("stderr_truncated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut lines = Vec::new();
    for line in stdout.lines() {
        lines.push(BashOutputLine {
            stream: BashStream::Stdout,
            text: line.to_string(),
        });
    }
    for line in stderr.lines().filter(|l| !l.is_empty()) {
        lines.push(BashOutputLine {
            stream: BashStream::Stderr,
            text: line.to_string(),
        });
    }
    Some(ParsedBashOutput {
        exit_code,
        timed_out,
        output_truncated: stdout_trunc || stderr_trunc,
        lines,
    })
}

#[must_use]
pub fn viewer_title(viewer: &BashViewerState) -> String {
    let prefix = if viewer.user_mode { "! bash" } else { "bash" };
    let status = if viewer.timed_out {
        "timeout".to_string()
    } else if viewer.is_error {
        "erreur".into()
    } else {
        match viewer.exit_code {
            Some(0) => "ok".into(),
            Some(code) => format!("exit {code}"),
            None => "ok".into(),
        }
    };
    let trunc = if viewer.output_truncated {
        " · tronqué"
    } else {
        ""
    };
    format!(
        " {prefix} — $ {} [{status}{trunc}] ",
        truncate_cmd(&viewer.command, 64)
    )
}

#[must_use]
pub fn viewer_footer(scroll_top: usize, total: usize, visible: usize) -> String {
    if total == 0 {
        return " Esc / e fermer ".to_string();
    }
    let end = (scroll_top + visible).min(total);
    format!(
        " {}/{} lignes · PgUp/PgDown · Esc fermer ",
        scroll_top + 1,
        end
    )
}

fn truncate_cmd(cmd: &str, max: usize) -> String {
    if cmd.len() <= max {
        return cmd.to_string();
    }
    format!("{}…", &cmd[..max])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stdout_stderr() {
        let out = serde_json::json!({
            "command": "echo hi",
            "exit_code": 1,
            "stdout": "ok\n",
            "stderr": "warn",
            "stdout_truncated": false,
            "stderr_truncated": false
        });
        let v = BashViewerState::from_output(ToolUseId::new(), &out, false, false, None).unwrap();
        assert_eq!(v.lines.len(), 2);
        assert_eq!(v.lines[0].stream, BashStream::Stdout);
        assert_eq!(v.lines[1].stream, BashStream::Stderr);
    }

    #[test]
    fn builds_viewer_with_command() {
        let out = serde_json::json!({
            "command": "ls",
            "exit_code": 0,
            "stdout": "a\n",
            "stderr": ""
        });
        let v = BashViewerState::from_output(ToolUseId::new(), &out, false, false, None).unwrap();
        assert_eq!(v.command, "ls");
        assert!(!v.user_mode);
    }
}
