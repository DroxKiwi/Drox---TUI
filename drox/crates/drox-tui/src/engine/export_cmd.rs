//! `/export` — export texte brut du transcript.

use std::path::PathBuf;

use anyhow::Context;
use drox_types::{Content, Message, Role};

use super::EngineRuntime;

impl EngineRuntime {
    /// Exporte la conversation courante vers un fichier texte dans le workspace.
    pub async fn export_transcript(
        &self,
        filename_arg: &str,
    ) -> anyhow::Result<ExportReport> {
        let messages = self.load_history().await;
        if messages.is_empty() {
            anyhow::bail!("transcript vide — rien à exporter");
        }
        let body = messages_to_plain_text(&messages);
        let filename = resolve_export_filename(filename_arg, &messages);
        let path = self.workspace.join(filename);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent.as_std_path())
                .await
                .context("création répertoire export")?;
        }
        tokio::fs::write(path.as_std_path(), &body)
            .await
            .context("écriture export")?;
        Ok(ExportReport {
            path,
            bytes: body.len(),
            message_count: messages.len(),
        })
    }
}

/// Fichier écrit par `/export`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub path: camino::Utf8PathBuf,
    pub bytes: usize,
    pub message_count: usize,
}

/// Rendu texte brut (sans ANSI) pour export fichier.
#[must_use]
pub fn messages_to_plain_text(messages: &[Message]) -> String {
    let mut out = String::new();
    for msg in messages {
        match msg.role {
            Role::System => {
                let text = Content::collapse_text(&msg.content).trim().to_string();
                if text.is_empty() {
                    continue;
                }
                out.push_str("=== System ===\n");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            Role::User => {
                let text = user_plain_text(msg);
                if text.trim().is_empty() {
                    continue;
                }
                out.push_str("=== User ===\n");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            Role::Assistant => {
                append_assistant(msg, &mut out);
            }
            Role::Tool => {
                append_tool(msg, &mut out);
            }
        }
    }
    out.trim_end().to_string() + "\n"
}

fn user_plain_text(msg: &Message) -> String {
    let mut parts = Vec::new();
    for block in &msg.content {
        match block {
            Content::Text { text } if !text.trim().is_empty() => parts.push(text.trim().to_string()),
            Content::Image { mime, .. } => parts.push(format!("[image {mime}]")),
            _ => {}
        }
    }
    parts.join("\n")
}

fn append_assistant(msg: &Message, out: &mut String) {
    let mut wrote_header = false;
    for block in &msg.content {
        match block {
            Content::Text { text } if !text.trim().is_empty() => {
                if !wrote_header {
                    out.push_str("=== Assistant ===\n");
                    wrote_header = true;
                }
                out.push_str(text.trim());
                out.push('\n');
            }
            Content::ToolUse { name, input, .. } => {
                if !wrote_header {
                    out.push_str("=== Assistant ===\n");
                    wrote_header = true;
                }
                out.push_str(&format!("[tool {name}]\n"));
                if let Ok(pretty) = serde_json::to_string_pretty(input) {
                    out.push_str(&pretty);
                    out.push('\n');
                }
            }
            _ => {}
        }
    }
    if wrote_header {
        out.push('\n');
    }
}

fn append_tool(msg: &Message, out: &mut String) {
    for block in &msg.content {
        let Content::ToolResult {
            content,
            is_error,
            ..
        } = block
        else {
            continue;
        };
        let tag = if *is_error { "Tool (error)" } else { "Tool" };
        out.push_str(&format!("=== {tag} ===\n"));
        out.push_str(content.trim());
        out.push_str("\n\n");
    }
}

fn resolve_export_filename(filename_arg: &str, messages: &[Message]) -> String {
    let trimmed = filename_arg.trim();
    if !trimmed.is_empty() {
        return normalize_filename(trimmed);
    }
    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string();
    let first = extract_first_prompt(messages);
    if first.is_empty() {
        return format!("conversation-{timestamp}.txt");
    }
    let slug = sanitize_filename(&first);
    if slug.is_empty() {
        format!("conversation-{timestamp}.txt")
    } else {
        format!("{timestamp}-{slug}.txt")
    }
}

fn normalize_filename(name: &str) -> String {
    let path = PathBuf::from(name);
    let base = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    if base.ends_with(".txt") {
        base.to_string()
    } else {
        let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
        format!("{stem}.txt")
    }
}

fn extract_first_prompt(messages: &[Message]) -> String {
    let Some(msg) = messages.iter().find(|m| m.role == Role::User) else {
        return String::new();
    };
    let text = user_plain_text(msg);
    let line = text.lines().next().unwrap_or(&text).trim();
    if line.len() > 50 {
        format!("{}…", &line[..49])
    } else {
        line.to_string()
    }
}

fn sanitize_filename(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut slug = String::new();
    let mut prev_hyphen = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_hyphen = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !slug.is_empty() && !prev_hyphen {
                slug.push('-');
                prev_hyphen = true;
            }
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_includes_roles() {
        let messages = vec![
            Message::user("fix bug"),
            Message::assistant("ok"),
        ];
        let text = messages_to_plain_text(&messages);
        assert!(text.contains("=== User ==="));
        assert!(text.contains("fix bug"));
        assert!(text.contains("=== Assistant ==="));
    }

    #[test]
    fn sanitize_slug() {
        assert_eq!(sanitize_filename("Hello World!"), "hello-world");
    }

    #[test]
    fn default_filename_uses_prompt() {
        let messages = vec![Message::user("Add tests for export")];
        let name = resolve_export_filename("", &messages);
        assert!(name.ends_with(".txt"));
        assert!(name.contains("add-tests-for-export"));
    }
}
