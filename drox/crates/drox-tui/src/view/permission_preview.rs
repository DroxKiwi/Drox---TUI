//! Preview diff / bash / URL / plan pour dialogues permission.

use camino::Utf8Path;
use drox_bash::BashCommandKind;
use drox_tools::{preview_file_edit_diff, preview_file_write_diff, preview_notebook_edit_diff};
use serde_json::Value;

use super::bash_output::{bash_permission_preview, BashPermissionPreview};

/// Corps d'un preview affiché dans la modal permission.
#[derive(Debug, Clone)]
pub struct PermissionPreview {
    pub tool_name: String,
    pub kind: PermissionPreviewBody,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PermissionPreviewBody {
    FileDiff {
        path_label: Option<String>,
        lines: Vec<String>,
    },
    Bash(BashPermissionPreview),
    Url {
        url: String,
    },
    PathAction {
        action: String,
        path: String,
        detail: Option<String>,
    },
    PathPair {
        source: String,
        destination: String,
    },
    TextBlock {
        title: String,
        lines: Vec<String>,
    },
}

const MAX_DIFF_LINES: usize = 28;
const MAX_TEXT_LINES: usize = 24;

/// Parse le prompt produit par `drox_engine::confirm_with_user`.
#[must_use]
pub fn parse_permission_prompt(prompt: &str) -> Option<(String, String, Value)> {
    const MARKER: &str = "\n\nAllow this `";
    let idx = prompt.find(MARKER)?;
    let permission_message = prompt[..idx].trim().to_string();
    let rest = &prompt[idx + MARKER.len()..];
    let end = rest.find("` call?\nArgs:\n")?;
    let tool_name = rest[..end].to_string();
    let args_json = &rest[end + "` call?\nArgs:\n".len()..];
    let args = serde_json::from_str(args_json).ok()?;
    Some((tool_name, permission_message, args))
}

/// Preview pour l'approbation de plan (`exit_plan_mode` via `UserAsker`).
#[must_use]
pub fn parse_plan_approval_preview(prompt: &str) -> Option<PermissionPreview> {
    const START: &str = "Plan proposé :\n\n";
    const END: &str = "\n\nL'approuves-tu ?";
    let plan = prompt.strip_prefix(START)?.split(END).next()?.trim();
    if plan.is_empty() {
        return None;
    }
    Some(PermissionPreview {
        tool_name: "exit_plan_mode".into(),
        kind: PermissionPreviewBody::TextBlock {
            title: "Plan proposé".into(),
            lines: wrap_text_lines(plan, MAX_TEXT_LINES),
        },
        error: None,
    })
}

/// Calcule le preview si l'appel permission le supporte.
#[must_use]
pub fn compute_permission_preview(workspace: &Utf8Path, prompt: &str) -> Option<PermissionPreview> {
    let (tool_name, _message, args) = parse_permission_prompt(prompt)?;
    let preview = match tool_name.as_str() {
        "file_edit" => file_diff_preview(workspace, &tool_name, &args, preview_file_edit_diff),
        "file_write" => file_diff_preview(workspace, &tool_name, &args, preview_file_write_diff),
        "notebook_edit" => {
            file_diff_preview(workspace, &tool_name, &args, preview_notebook_edit_diff)
        }
        "bash" => match bash_permission_preview(&args) {
            Ok(body) => PermissionPreview {
                tool_name,
                kind: PermissionPreviewBody::Bash(body),
                error: None,
            },
            Err(msg) => PermissionPreview {
                tool_name,
                kind: PermissionPreviewBody::Bash(BashPermissionPreview {
                    command: String::new(),
                    description: None,
                    segments: Vec::new(),
                }),
                error: Some(msg),
            },
        },
        "delete_path" => {
            let path = extract_path_label(&args).unwrap_or_else(|| "?".into());
            let recursive = args
                .get("recursive")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            PermissionPreview {
                tool_name,
                kind: PermissionPreviewBody::PathAction {
                    action: "Supprimer".into(),
                    path,
                    detail: Some(if recursive {
                        "récursif".into()
                    } else {
                        "non récursif (dossier vide seulement)".into()
                    }),
                },
                error: None,
            }
        }
        "copy_path" => PermissionPreview {
            tool_name,
            kind: PermissionPreviewBody::PathPair {
                source: args
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
                destination: args
                    .get("destination")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
            },
            error: None,
        },
        "web_fetch" => PermissionPreview {
            tool_name,
            kind: PermissionPreviewBody::Url {
                url: args
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
            },
            error: None,
        },
        "skill_read" => PermissionPreview {
            tool_name,
            kind: PermissionPreviewBody::TextBlock {
                title: "Charger skill".into(),
                lines: vec![format!(
                    "name: {}",
                    args.get("name").and_then(Value::as_str).unwrap_or("?")
                )],
            },
            error: None,
        },
        name if name.starts_with("mcp__") => {
            let pretty = serde_json::to_string_pretty(&args).unwrap_or_else(|_| "{}".into());
            PermissionPreview {
                tool_name: name.to_string(),
                kind: PermissionPreviewBody::TextBlock {
                    title: "Appel MCP".into(),
                    lines: wrap_text_lines(&pretty, 12),
                },
                error: None,
            }
        }
        "mcp_call" => PermissionPreview {
            tool_name,
            kind: PermissionPreviewBody::TextBlock {
                title: format!(
                    "MCP {} :: {}",
                    args.get("server").and_then(Value::as_str).unwrap_or("?"),
                    args.get("tool").and_then(Value::as_str).unwrap_or("?")
                ),
                lines: args
                    .get("arguments")
                    .map(|v| wrap_text_lines(&serde_json::to_string_pretty(v).unwrap_or_default(), 10))
                    .unwrap_or_default(),
            },
            error: None,
        },
        _ => return None,
    };
    Some(preview)
}

fn file_diff_preview(
    workspace: &Utf8Path,
    tool_name: &str,
    args: &Value,
    preview_fn: fn(&Utf8Path, &Value) -> Result<String, drox_tools::ToolError>,
) -> PermissionPreview {
    let path_label = extract_path_label(args);
    match preview_fn(workspace, args) {
        Ok(diff) => PermissionPreview {
            tool_name: tool_name.to_string(),
            kind: PermissionPreviewBody::FileDiff {
                path_label,
                lines: truncate_lines(&diff),
            },
            error: None,
        },
        Err(err) => PermissionPreview {
            tool_name: tool_name.to_string(),
            kind: PermissionPreviewBody::FileDiff {
                path_label,
                lines: Vec::new(),
            },
            error: Some(err.to_string()),
        },
    }
}

#[must_use]
pub fn preview_body_line_count(preview: &PermissionPreview) -> usize {
    if preview.error.is_some() {
        return 2;
    }
    match &preview.kind {
        PermissionPreviewBody::FileDiff { lines, .. } => lines.len().saturating_add(2),
        PermissionPreviewBody::Bash(body) => {
            body.segments.len().saturating_add(3) + usize::from(body.description.is_some())
        }
        PermissionPreviewBody::Url { .. } => 3,
        PermissionPreviewBody::PathAction { .. } => 4,
        PermissionPreviewBody::PathPair { .. } => 4,
        PermissionPreviewBody::TextBlock { lines, .. } => lines.len().saturating_add(2),
    }
}

#[must_use]
pub fn bash_kind_color(kind: BashCommandKind) -> ratatui::style::Color {
    use ratatui::style::Color;
    match kind {
        BashCommandKind::ReadOnly => Color::Green,
        BashCommandKind::Mutating => Color::Yellow,
        BashCommandKind::Network => Color::Cyan,
        BashCommandKind::Destructive => Color::Red,
        BashCommandKind::Unknown => Color::Gray,
        _ => Color::Gray,
    }
}

fn extract_path_label(args: &Value) -> Option<String> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .or_else(|| args.get("notebook_path"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn truncate_lines(diff: &str) -> Vec<String> {
    let mut lines: Vec<String> = diff.lines().map(str::to_string).collect();
    if lines.len() > MAX_DIFF_LINES {
        lines.truncate(MAX_DIFF_LINES);
        lines.push("… (diff tronqué)".into());
    }
    lines
}

fn wrap_text_lines(text: &str, max_lines: usize) -> Vec<String> {
    let mut out: Vec<String> = text.lines().map(str::to_string).collect();
    if out.len() > max_lines {
        out.truncate(max_lines);
        out.push("… (tronqué)".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_permission_prompt_round_trip() {
        let prompt = "Drox requests permission.\n\nAllow this `file_edit` call?\nArgs:\n{\n  \"path\": \"a.rs\"\n}";
        let (tool, msg, args) = parse_permission_prompt(prompt).unwrap();
        assert_eq!(tool, "file_edit");
        assert_eq!(msg, "Drox requests permission.");
        assert_eq!(args["path"], "a.rs");
    }

    #[test]
    fn compute_bash_preview() {
        let prompt = "msg\n\nAllow this `bash` call?\nArgs:\n{\"command\":\"echo hi\"}";
        let preview = compute_permission_preview(Utf8Path::new("."), prompt).unwrap();
        assert_eq!(preview.tool_name, "bash");
        assert!(matches!(preview.kind, PermissionPreviewBody::Bash(_)));
    }

    #[test]
    fn compute_web_fetch_preview() {
        let prompt = "msg\n\nAllow this `web_fetch` call?\nArgs:\n{\"url\":\"https://example.com\"}";
        let preview = compute_permission_preview(Utf8Path::new("."), prompt).unwrap();
        assert!(matches!(preview.kind, PermissionPreviewBody::Url { .. }));
    }

    #[test]
    fn parse_plan_approval() {
        let prompt = "Plan proposé :\n\n## Étape 1\nFaire X\n\nL'approuves-tu ?";
        let preview = parse_plan_approval_preview(prompt).unwrap();
        assert!(matches!(preview.kind, PermissionPreviewBody::TextBlock { .. }));
    }

    #[test]
    fn compute_preview_unknown_tool_returns_none() {
        let prompt = "msg\n\nAllow this `grep` call?\nArgs:\n{}";
        assert!(compute_permission_preview(Utf8Path::new("."), prompt).is_none());
    }
}
