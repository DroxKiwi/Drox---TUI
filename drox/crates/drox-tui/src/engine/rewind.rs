//! `/rewind` — sélection d'un message utilisateur et troncature du transcript.

use anyhow::Context;
use drox_types::{Content, Message, Role};

use super::commands::rewrite_transcript;
use super::EngineRuntime;

/// Cible rembobinable dans le transcript.
#[derive(Debug, Clone)]
pub struct RewindChoice {
    /// Index du message `user` dans le transcript complet.
    pub message_index: usize,
    /// Ligne affichée dans le sélecteur.
    pub label: String,
    /// Texte à restaurer dans le composer.
    pub restore_text: String,
}

impl EngineRuntime {
    /// Liste les messages utilisateur sélectionnables (ordre chronologique).
    pub async fn list_rewind_choices(&self) -> anyhow::Result<Vec<RewindChoice>> {
        let messages = self.load_history().await;
        Ok(build_rewind_choices(&messages))
    }

    /// Tronque le transcript avant `message_index` et renvoie le texte restauré.
    pub async fn rewind_to_message_index(
        &self,
        message_index: usize,
    ) -> anyhow::Result<RewindReport> {
        let messages = self.load_history().await;
        let Some(choice) = build_rewind_choices(&messages)
            .into_iter()
            .find(|c| c.message_index == message_index)
        else {
            anyhow::bail!("message utilisateur introuvable (index {message_index})");
        };
        if message_index > messages.len() {
            anyhow::bail!("index transcript hors limites");
        }
        let kept = message_index;
        let removed = messages.len().saturating_sub(message_index);
        let truncated = messages[..message_index].to_vec();
        rewrite_transcript(&self.transcript_path(), &truncated)
            .await
            .context("écriture transcript rembobiné")?;
        Ok(RewindReport {
            messages_kept: kept,
            messages_removed: removed,
            restore_text: choice.restore_text,
        })
    }
}

/// Résumé d'un rembobinage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewindReport {
    pub messages_kept: usize,
    pub messages_removed: usize,
    pub restore_text: String,
}

fn build_rewind_choices(messages: &[Message]) -> Vec<RewindChoice> {
    let mut out = Vec::new();
    let mut user_no = 0usize;
    for (idx, msg) in messages.iter().enumerate() {
        if msg.role != Role::User {
            continue;
        }
        let text = user_display_text(msg);
        if text.trim().is_empty() {
            continue;
        }
        user_no += 1;
        let preview = truncate_line(&text, 56);
        out.push(RewindChoice {
            message_index: idx,
            label: format!("#{user_no} {preview}"),
            restore_text: text,
        });
    }
    out
}

fn user_display_text(msg: &Message) -> String {
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

fn truncate_line(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or(text).trim();
    if line.len() <= max {
        line.to_string()
    } else {
        format!("{}…", &line[..max.saturating_sub(1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_user_messages_only() {
        let messages = vec![
            Message::system("sys"),
            Message::user("hello"),
            Message::assistant("hi"),
            Message::user("second prompt"),
        ];
        let choices = build_rewind_choices(&messages);
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].message_index, 1);
        assert_eq!(choices[1].message_index, 3);
        assert_eq!(choices[0].restore_text, "hello");
    }
}
