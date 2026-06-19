//! Types de messages échangés avec le LLM.
//!
//! Sprint 1.4 : `Content` accepte `ToolUse` (demande du modèle) et
//! `ToolResult` (réponse de l'exécuteur de tools).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::ToolUseId;

/// Rôle d'un message dans une conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Bloc de contenu d'un message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Content {
    /// Texte brut.
    Text { text: String },
    /// Image attachée par l'utilisateur (input multimodal).
    ///
    /// `data` doit être la base64 *brute* (sans préfixe `data:...;base64,`).
    /// `mime` est conservé pour les providers qui en ont besoin (`Anthropic`,
    /// `OpenAI`). Ollama n'utilise que `data`.
    Image { mime: String, data: String },
    /// Demande d'exécution d'un tool par l'assistant.
    ToolUse {
        id: ToolUseId,
        name: String,
        input: Value,
    },
    /// Résultat d'exécution d'un tool, renvoyé au modèle.
    ToolResult {
        tool_use_id: ToolUseId,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

impl Content {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Construit un bloc image à partir d'un type MIME et d'une charge utile
    /// base64 brute (sans préfixe `data:...`).
    pub fn image(mime: impl Into<String>, data_base64: impl Into<String>) -> Self {
        Self::Image {
            mime: mime.into(),
            data: data_base64.into(),
        }
    }

    /// Concatène le texte brut de tous les blocs `Text`. Ignore les autres
    /// variantes (utile pour les providers wire-format text-only comme Ollama
    /// `/api/chat`).
    pub fn collapse_text(content: &[Self]) -> String {
        let mut out = String::new();
        for block in content {
            if let Self::Text { text } = block {
                out.push_str(text);
            }
        }
        out
    }
}

/// Message dans une conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
}

impl Message {
    pub const fn new(role: Role, content: Vec<Content>) -> Self {
        Self { role, content }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self::new(Role::System, vec![Content::text(text)])
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self::new(Role::User, vec![Content::text(text)])
    }

    /// Construit un message `user` avec une liste arbitraire de blocs
    /// (texte + images, par exemple). Utilisé pour les prompts multimodaux.
    pub const fn user_with_blocks(blocks: Vec<Content>) -> Self {
        Self::new(Role::User, blocks)
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::new(Role::Assistant, vec![Content::text(text)])
    }

    /// Construit un message `tool` avec un unique bloc `ToolResult`.
    pub fn tool_result(id: ToolUseId, content: impl Into<String>, is_error: bool) -> Self {
        Self::new(
            Role::Tool,
            vec![Content::ToolResult {
                tool_use_id: id,
                content: content.into(),
                is_error,
            }],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_text_joins_text_blocks() {
        let blocks = vec![Content::text("hello "), Content::text("world")];
        assert_eq!(Content::collapse_text(&blocks), "hello world");
    }

    #[test]
    fn collapse_text_ignores_non_text_blocks() {
        let blocks = vec![
            Content::text("hi "),
            Content::ToolUse {
                id: ToolUseId::new(),
                name: "x".into(),
                input: serde_json::json!({}),
            },
            Content::text("there"),
        ];
        assert_eq!(Content::collapse_text(&blocks), "hi there");
    }

    #[test]
    fn image_round_trip_via_serde() {
        let block = Content::image("image/png", "AAAA");
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["mime"], "image/png");
        assert_eq!(json["data"], "AAAA");
        let back: Content = serde_json::from_value(json).unwrap();
        match back {
            Content::Image { mime, data } => {
                assert_eq!(mime, "image/png");
                assert_eq!(data, "AAAA");
            }
            other => panic!("expected Image, got {other:?}"),
        }
    }

    #[test]
    fn user_with_blocks_preserves_order() {
        let m = Message::user_with_blocks(vec![
            Content::text("regarde:"),
            Content::image("image/png", "AAAA"),
        ]);
        assert_eq!(m.role, Role::User);
        assert_eq!(m.content.len(), 2);
        assert!(matches!(&m.content[0], Content::Text { text } if text == "regarde:"));
        assert!(matches!(&m.content[1], Content::Image { mime, .. } if mime == "image/png"));
    }

    #[test]
    fn role_serde_round_trip() {
        for role in [Role::System, Role::User, Role::Assistant, Role::Tool] {
            let json = serde_json::to_string(&role).unwrap();
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }
}
