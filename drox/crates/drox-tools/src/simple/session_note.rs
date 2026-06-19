//! Tool `session_note` — épingle une note de travail dans le run courant.
//!
//! Sprint M1 (mémoire unifiée). Le modèle l'appelle quand il veut **fixer
//! une information** qui devrait survivre au-delà du contexte courant :
//! décision technique non triviale, choix d'API, hypothèse à vérifier
//! plus tard, point bloquant.
//!
//! Le contenu est stocké en RAM via [`SessionNotesHandle`] (cf.
//! [`crate::session_notes`]). À la fin du run (`[phase: done]`), le
//! moteur drainera ces notes et les injectera dans le résumé de
//! compaction persisté dans `.drox/memory/sessions/`.
//!
//! ## Quand l'utiliser (pour le modèle)
//!
//! - **OUI** : « j'ai choisi sqlx plutôt que diesel parce que tokio-native »,
//!   « la table users a une contrainte UNIQUE sur email », « TODO :
//!   vérifier le comportement du retry sur erreur 429 ».
//! - **NON** : narrer ce que le tool vient de retourner, expliquer le
//!   plan, dire « j'ai fini ». Ces choses-là ressortiront naturellement
//!   du résumé de compaction.
//!
//! ## Garanties
//!
//! - Effet : append-only en RAM. Aucune écriture filesystem ni réseau.
//! - Coût : ~µs. Aucune raison d'asker la permission utilisateur (le
//!   moteur l'auto-allow côté `PermissionPolicy`).
//! - Borne : `content` ≤ 500 chars, refusé sinon avec message clair.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

/// Limite dure imposée au contenu d'une note — sécurité anti-dérive et
/// anti-flood. Une note doit rester un **rappel** court, pas une rédaction.
const MAX_NOTE_LEN: usize = 500;

/// Payload reçu par le tool `session_note`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionNoteInput {
    /// Contenu de la note (≤ 500 chars). Texte libre, mais privilégie une
    /// phrase télégraphique : « Décision X parce que Y », « TODO : vérifier
    /// Z », « Hypothèse : … ».
    pub content: String,
}

/// Tool local `session_note`.
pub struct SessionNoteTool;

#[async_trait]
impl Tool for SessionNoteTool {
    fn name(&self) -> &str {
        "session_note"
    }

    fn description(&self) -> &str {
        "Épingle une note de travail (≤ 500 chars) qui sera intégrée au \
         résumé persistant de la session à la fin du run. À utiliser pour \
         fixer une décision technique non triviale, une hypothèse à \
         vérifier, ou un point bloquant — PAS pour narrer le tool précédent \
         ni pour annoncer un plan. Effet : append en mémoire, zéro I/O. \
         Format : {\"content\": \"…\"}."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(SessionNoteInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: SessionNoteInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "session_note: payload JSON invalide ({e}). \
                 Format attendu : {{\"content\": \"…\"}}.",
            ))
        })?;
        let content = args.content.trim().to_string();
        if content.is_empty() {
            return Err(ToolError::invalid_args(
                "session_note: `content` is empty after trim",
            ));
        }
        if content.chars().count() > MAX_NOTE_LEN {
            return Err(ToolError::invalid_args(format!(
                "session_note: `content` too long ({} chars, max {MAX_NOTE_LEN}). \
                 Use a shorter telegraphic note, or split into several calls.",
                content.chars().count()
            )));
        }
        let Some(handle) = ctx.session_notes.as_ref() else {
            return Err(ToolError::invalid_args(
                "session_note: tool unavailable in this context (no session memory backend wired)",
            ));
        };
        let total = handle.push(content);
        Ok(json!({
            "noted": true,
            "total_notes": total,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_notes::SessionNotesHandle;
    use camino::Utf8PathBuf;

    fn ctx_with_notes(handle: SessionNotesHandle) -> ToolContext {
        ToolContext::new(Utf8PathBuf::from("/tmp"), false).with_session_notes(handle)
    }

    #[tokio::test]
    async fn appends_note_and_returns_total() {
        let h = SessionNotesHandle::new();
        let ctx = ctx_with_notes(h.clone());
        let out = SessionNoteTool
            .execute(&ctx, json!({ "content": "Décision sqlx > diesel : compat tokio" }))
            .await
            .unwrap();
        assert_eq!(out["noted"], true);
        assert_eq!(out["total_notes"], 1);
        assert_eq!(h.len(), 1);
        assert_eq!(h.snapshot()[0].content, "Décision sqlx > diesel : compat tokio");
    }

    #[tokio::test]
    async fn rejects_empty_content() {
        let h = SessionNotesHandle::new();
        let ctx = ctx_with_notes(h.clone());
        let err = SessionNoteTool
            .execute(&ctx, json!({ "content": "  \n  " }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains("empty after trim")),
            "got {err:?}"
        );
        assert!(h.is_empty());
    }

    #[tokio::test]
    async fn rejects_oversize_content() {
        let h = SessionNotesHandle::new();
        let ctx = ctx_with_notes(h.clone());
        let big = "x".repeat(MAX_NOTE_LEN + 1);
        let err = SessionNoteTool
            .execute(&ctx, json!({ "content": big }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains("too long")),
            "got {err:?}"
        );
        assert!(h.is_empty());
    }

    #[tokio::test]
    async fn accepts_content_at_exact_limit() {
        let h = SessionNotesHandle::new();
        let ctx = ctx_with_notes(h.clone());
        let exact = "x".repeat(MAX_NOTE_LEN);
        SessionNoteTool
            .execute(&ctx, json!({ "content": exact }))
            .await
            .unwrap();
        assert_eq!(h.len(), 1);
    }

    #[tokio::test]
    async fn errors_clearly_when_handle_missing() {
        let ctx = ToolContext::new(Utf8PathBuf::from("/tmp"), false);
        let err = SessionNoteTool
            .execute(&ctx, json!({ "content": "abc" }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains("unavailable")),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn multiple_notes_accumulate() {
        let h = SessionNotesHandle::new();
        let ctx = ctx_with_notes(h.clone());
        for i in 0..3 {
            let out = SessionNoteTool
                .execute(&ctx, json!({ "content": format!("note {i}") }))
                .await
                .unwrap();
            assert_eq!(out["total_notes"], i + 1);
        }
        assert_eq!(h.snapshot().len(), 3);
    }

    #[tokio::test]
    async fn rejects_malformed_payload() {
        let h = SessionNotesHandle::new();
        let ctx = ctx_with_notes(h.clone());
        let err = SessionNoteTool
            .execute(&ctx, json!({ "wrong": "key" }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArgs(ref m) if m.contains("session_note")),
            "got {err:?}"
        );
    }
}
