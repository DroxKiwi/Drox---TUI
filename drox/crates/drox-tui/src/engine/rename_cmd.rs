//! `/rename` — titre personnalisé de session (`ses_*.meta.json`).

use anyhow::Context;
use camino::Utf8Path;
use drox_session::{
    display_title, read_session_meta, read_transcript, session_meta_path, write_session_meta,
    SessionMeta,
};
use drox_types::{Content, Message, SessionId};

/// Renomme la session courante. Sans argument, déduit un titre du premier message user.
pub async fn rename_session(
    sessions_dir: &Utf8Path,
    session_id: &str,
    transcript_path: &Utf8Path,
    requested: Option<&str>,
) -> anyhow::Result<String> {
    let sid = SessionId::from_string(session_id.to_string());
    let meta_path = session_meta_path(sessions_dir, &sid);

    let title = match requested.map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => name.to_string(),
        None => derive_title_from_transcript(transcript_path)
            .await
            .context("aucun message utilisateur pour générer un titre — précisez /rename <nom>")?,
    };

    let meta = SessionMeta {
        custom_title: Some(title.clone()),
    };
    write_session_meta(&meta_path, &meta)
        .await
        .context("écriture meta session")?;

    Ok(title)
}

/// Titre affichable pour une session (meta ou id).
pub async fn load_display_title(sessions_dir: &Utf8Path, session_id: &str) -> String {
    let sid = SessionId::from_string(session_id.to_string());
    let path = session_meta_path(sessions_dir, &sid);
    let meta = read_session_meta(&path).await;
    display_title(session_id, meta.as_ref())
}

async fn derive_title_from_transcript(transcript_path: &Utf8Path) -> Option<String> {
    let messages = read_transcript(transcript_path).await.ok()?;
    first_user_line(&messages).map(|s| truncate_title(&s, 64))
}

fn first_user_line(messages: &[Message]) -> Option<String> {
    for msg in messages {
        if msg.role != drox_types::Role::User {
            continue;
        }
        let text = Content::collapse_text(&msg.content).trim().to_string();
        if !text.is_empty() && !text.starts_with('/') {
            return Some(text.replace('\n', " "));
        }
    }
    None
}

fn truncate_title(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_title() {
        let long = "a".repeat(80);
        assert!(truncate_title(&long, 20).chars().count() <= 20);
    }
}
