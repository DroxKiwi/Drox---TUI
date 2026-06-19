//! Formatage partagé des sorties bash (fil + permissions).

use drox_bash::{
    auto_deny_message, destructive_hint, kind_of_segment, split_command_segments, BashCommandKind,
};
use drox_types::ToolUseId;
use serde_json::Value;

/// Segment bash classifié pour preview permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashSegmentPreview {
    pub text: String,
    pub kind: BashCommandKind,
    pub warning: Option<String>,
}

/// Construit le preview bash à partir des arguments tool.
#[must_use]
pub fn bash_permission_preview(args: &Value) -> Result<BashPermissionPreview, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "commande bash vide".to_string())?;
    let description = args
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let raw_segments = split_command_segments(command)
        .unwrap_or_else(|_| vec![command.to_string()]);
    let segments = raw_segments
        .into_iter()
        .map(|text| {
            let kind = kind_of_segment(&text);
            let warning = destructive_hint(&text)
                .or_else(|| auto_deny_message(&text))
                .map(str::to_string);
            BashSegmentPreview {
                text,
                kind,
                warning,
            }
        })
        .collect();

    Ok(BashPermissionPreview {
        command: command.to_string(),
        description,
        segments,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashPermissionPreview {
    pub command: String,
    pub description: Option<String>,
    pub segments: Vec<BashSegmentPreview>,
}

#[must_use]
pub const fn bash_kind_label(kind: BashCommandKind) -> &'static str {
    match kind {
        BashCommandKind::ReadOnly => "lecture",
        BashCommandKind::Mutating => "mutation",
        BashCommandKind::Network => "réseau",
        BashCommandKind::Destructive => "destructif",
        BashCommandKind::Unknown => "inconnu",
        _ => "inconnu",
    }
}

/// Lignes affichées pour un `ToolFinish` bash dans le fil.
#[must_use]
pub fn format_bash_finish_lines(id: &ToolUseId, output: &Value, is_error: bool, expanded: bool) -> Vec<String> {
    let timed_out = output
        .get("timed_out")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let exit = output.get("exit_code");
    let status = if is_error {
        "erreur"
    } else if timed_out {
        "timeout"
    } else {
        match exit.and_then(Value::as_i64) {
            Some(0) => "ok",
            Some(code) => return vec![format!("◂ bash ({id}) [exit {code}]")],
            None => "ok",
        }
    };
    let mut lines = vec![format!("◂ bash ({id}) [{status}]")];
    let stdout = output
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or("");
    let stderr = output
        .get("stderr")
        .and_then(Value::as_str)
        .unwrap_or("");
    let max_lines = if expanded { 120 } else { 24 };
    let max_chars = if expanded { 12_000 } else { 2_000 };
    let mut total = 0usize;
    for line in stdout.lines().chain(stderr.lines().filter(|l| !l.is_empty())) {
        if lines.len() > max_lines || total >= max_chars {
            lines.push(if expanded {
                "  … (sortie tronquée)".into()
            } else {
                "  … (e pour parcourir)".into()
            });
            break;
        }
        total += line.len();
        lines.push(format!("  {line}"));
    }
    lines
}

/// Lignes pour une sortie mode `!` (bash utilisateur).
#[must_use]
pub fn format_bash_mode_finish_lines(
    id: &ToolUseId,
    output: &Value,
    is_error: bool,
    expanded: bool,
) -> Vec<String> {
    let mut lines = format_bash_finish_lines(id, output, is_error, expanded);
    if let Some(first) = lines.first_mut() {
        if let Some(rest) = first.strip_prefix("◂ bash ") {
            *first = format!("◂ ! bash {rest}");
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bash_preview_splits_compound() {
        let preview = bash_permission_preview(&json!({ "command": "ls -la && rm -rf /tmp/x" })).unwrap();
        assert_eq!(preview.segments.len(), 2);
        assert_eq!(preview.segments[0].kind, BashCommandKind::ReadOnly);
        assert_eq!(preview.segments[1].kind, BashCommandKind::Destructive);
        assert!(preview.segments[1].warning.is_some());
    }

    #[test]
    fn bash_preview_rejects_empty() {
        assert!(bash_permission_preview(&json!({ "command": "  " })).is_err());
    }
}
