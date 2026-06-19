//! Rejeu d'un transcript `Message` → entrées du fil TUI.

use std::collections::HashMap;

use drox_engine::Phase;
use drox_types::{Content, Message, Role, ToolUseId};
use serde_json::Value;

use super::LogEntry;

const SYSTEM_DISPLAY_MAX: usize = 160;
const TOOL_BODY_MAX: usize = 200;

/// Convertit l'historique conversationnel en lignes affichables.
#[must_use]
pub fn messages_to_log_entries(messages: &[Message]) -> (Vec<LogEntry>, HashMap<ToolUseId, String>) {
    let mut entries = Vec::new();
    let mut tool_names = HashMap::new();

    for msg in messages {
        match msg.role {
            Role::System => push_system(msg, &mut entries),
            Role::User => push_user(msg, &mut entries),
            Role::Assistant => push_assistant(msg, &mut entries, &mut tool_names),
            Role::Tool => push_tool_results(msg, &mut entries, &tool_names),
        }
    }

    (entries, tool_names)
}

fn push_system(msg: &Message, entries: &mut Vec<LogEntry>) {
    let text = Content::collapse_text(&msg.content).trim().to_string();
    if text.is_empty() {
        return;
    }
    let display = if text.len() > SYSTEM_DISPLAY_MAX {
        format!("{}…", &text[..SYSTEM_DISPLAY_MAX])
    } else {
        text
    };
    entries.push(LogEntry::System {
        text: format!("[système] {display}"),
    });
}

fn push_user(msg: &Message, entries: &mut Vec<LogEntry>) {
    let mut parts = Vec::new();
    for block in &msg.content {
        match block {
            Content::Text { text } if !text.trim().is_empty() => parts.push(text.trim().to_string()),
            Content::Image { mime, .. } => parts.push(format!("[image {mime}]")),
            _ => {}
        }
    }
    if parts.is_empty() {
        return;
    }
    entries.push(LogEntry::User {
        text: parts.join("\n"),
    });
}

fn push_assistant(
    msg: &Message,
    entries: &mut Vec<LogEntry>,
    tool_names: &mut HashMap<ToolUseId, String>,
) {
    for block in &msg.content {
        match block {
            Content::Text { text } => push_assistant_text(text, entries),
            Content::ToolUse { id, name, input } => {
                tool_names.insert(id.clone(), name.clone());
                entries.push(LogEntry::ToolStart {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: input.clone(),
                });
            }
            _ => {}
        }
    }
}

fn push_assistant_text(text: &str, entries: &mut Vec<LogEntry>) {
    let mut paragraph = String::new();

    for line in text.lines() {
        if let Some(phase) = parse_phase_marker(line) {
            if !paragraph.trim().is_empty() {
                entries.push(LogEntry::Assistant {
                    text: std::mem::take(&mut paragraph),
                });
            }
            if phase != Phase::Done {
                entries.push(LogEntry::PhaseOpen { phase });
            }
            continue;
        }
        if legacy_removed_phase_marker(line) {
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push('\n');
        }
        paragraph.push_str(line);
    }

    if !paragraph.trim().is_empty() {
        entries.push(LogEntry::Assistant { text: paragraph });
    }
}

fn push_tool_results(
    msg: &Message,
    entries: &mut Vec<LogEntry>,
    tool_names: &HashMap<ToolUseId, String>,
) {
    for block in &msg.content {
        let Content::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = block
        else {
            continue;
        };
        let name = tool_names
            .get(tool_use_id)
            .cloned()
            .unwrap_or_else(|| "?".into());
        let output = tool_output_value(content);
        entries.push(LogEntry::ToolFinish {
            id: tool_use_id.clone(),
            name,
            output,
            is_error: *is_error,
        });
    }
}

fn tool_output_value(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|_| Value::String(truncate(body, TOOL_BODY_MAX)))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

/// Parse `[phase: nom]` sur une ligne entière (aligné moteur).
#[must_use]
fn parse_phase_marker(line: &str) -> Option<Phase> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let (head, raw_name) = body.split_once(':')?;
    if !head.trim().eq_ignore_ascii_case("phase") {
        return None;
    }
    let name = raw_name.trim().to_ascii_lowercase();
    let name = name.replace('_', "-").replace(' ', "-");
    match name.as_str() {
        "analyzing" | "analysis" | "survey" => Some(Phase::Analyzing),
        "reading" | "read" => Some(Phase::Reading),
        "clarifying" | "clarify" | "clarification" => Some(Phase::Clarifying),
        "planning" | "plan" => Some(Phase::Planning),
        "acting" | "act" | "action" => Some(Phase::Acting),
        "testing" | "test" | "tests" => Some(Phase::Testing),
        "verifying" | "verify" | "verification" => Some(Phase::Verifying),
        "answering" | "answer" | "respond" | "reply" | "response" => Some(Phase::Answering),
        "done" | "finish" | "finished" | "complete" | "completed" => Some(Phase::Done),
        _ => None,
    }
}

#[must_use]
fn legacy_removed_phase_marker(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(body) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return false;
    };
    let Some((head, raw_name)) = body.split_once(':') else {
        return false;
    };
    if !head.trim().eq_ignore_ascii_case("phase") {
        return false;
    }
    let name = raw_name.trim().to_ascii_lowercase();
    let name = name.replace('_', "-").replace(' ', "-");
    matches!(
        name.as_str(),
        "reasoning" | "reason" | "think" | "thought"
            | "next-move" | "nextmove" | "next" | "next-step"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::Message;

    #[test]
    fn user_and_assistant_round_trip() {
        let msgs = vec![
            Message::user("bonjour"),
            Message::assistant("[phase: answering]\nSalut !\n[phase: done]"),
        ];
        let (entries, _) = messages_to_log_entries(&msgs);
        assert!(entries.iter().any(|e| matches!(e, LogEntry::User { .. })));
        assert!(entries.iter().any(|e| matches!(e, LogEntry::PhaseOpen { phase: Phase::Answering })));
        assert!(entries.iter().any(|e| matches!(e, LogEntry::Assistant { text } if text.contains("Salut"))));
    }

    #[test]
    fn tool_use_and_result() {
        let id = ToolUseId::new();
        let msgs = vec![
            Message::new(
                Role::Assistant,
                vec![Content::ToolUse {
                    id: id.clone(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "echo hi"}),
                }],
            ),
            Message::tool_result(id.clone(), r#"{"stdout":"hi"}"#, false),
        ];
        let (entries, names) = messages_to_log_entries(&msgs);
        assert_eq!(names.get(&id).map(String::as_str), Some("bash"));
        assert!(entries.iter().any(|e| matches!(e, LogEntry::ToolStart { name, .. } if name == "bash")));
        assert!(entries.iter().any(|e| matches!(e, LogEntry::ToolFinish { name, .. } if name == "bash")));
    }
}
