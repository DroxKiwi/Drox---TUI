//! Compaction par résumé LLM.
//!
//! Découpe la conversation en deux : une fenêtre **ancienne** envoyée à un
//! [`Summarizer`] (typiquement un appel LLM), et une fenêtre **récente**
//! conservée intacte. Le résumé devient le premier message système de la
//! conversation compactée.
//!
//! L'implémentation par défaut de [`Summarizer`] basée sur un `LlmClient`
//! sera ajoutée plus tard quand l'agent en aura besoin ; cette crate ne se
//! couple pas à un provider concret pour pouvoir être testée à vide.

use async_trait::async_trait;
use drox_types::{Content, Message, Role};

use crate::error::ContextError;

/// Prompt système par défaut pour demander un résumé compact.
pub const DEFAULT_COMPACT_INSTRUCTIONS: &str = "Résume la conversation suivante en préservant : les objectifs de l'utilisateur, \
     les décisions techniques actées, les fichiers et chemins évoqués, les commandes \
     déjà exécutées et leurs résultats clés, et tout TODO en cours. Sois concis.";

/// Contrat d'un résumeur (par exemple un appel LLM).
#[async_trait]
pub trait Summarizer: Send + Sync {
    /// Produit un résumé textuel des messages fournis.
    async fn summarize(&self, messages: &[Message]) -> Result<String, ContextError>;
}

/// Configuration d'un cycle de compaction.
#[derive(Debug, Clone)]
pub struct CompactConfig {
    /// Nombre de messages les plus récents conservés tels quels.
    pub keep_recent_messages: usize,
    /// Préfixe inséré devant le résumé dans le message système consolidé.
    pub summary_prefix: String,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            keep_recent_messages: 4,
            summary_prefix: "Résumé de l'historique compacté :\n".into(),
        }
    }
}

/// Issue d'une compaction.
#[derive(Debug, Clone)]
pub struct CompactOutcome {
    /// Conversation compactée (résumé + queue récente).
    pub messages: Vec<Message>,
    /// Nombre de messages anciens absorbés dans le résumé.
    pub absorbed: usize,
}

/// Compacte une conversation en déléguant le résumé au `Summarizer`.
///
/// Si le nombre de messages est `≤ keep_recent_messages`, la conversation est
/// renvoyée inchangée (`absorbed = 0`).
pub async fn compact_conversation<S: Summarizer + ?Sized>(
    messages: &[Message],
    summarizer: &S,
    config: &CompactConfig,
) -> Result<CompactOutcome, ContextError> {
    if config.keep_recent_messages == 0 {
        return Err(ContextError::InvalidConfig(
            "keep_recent_messages must be > 0".into(),
        ));
    }
    if messages.len() <= config.keep_recent_messages {
        return Ok(CompactOutcome {
            messages: messages.to_vec(),
            absorbed: 0,
        });
    }

    let split = messages.len() - config.keep_recent_messages;
    let (older, recent) = messages.split_at(split);

    let summary = summarizer.summarize(older).await?;

    let summary_message = Message::new(
        Role::System,
        vec![Content::text(format!("{}{summary}", config.summary_prefix))],
    );

    let mut out = Vec::with_capacity(1 + recent.len());
    out.push(summary_message);
    out.extend_from_slice(recent);

    Ok(CompactOutcome {
        messages: out,
        absorbed: older.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Summarizer factice : concatène le texte des messages avec un séparateur.
    struct EchoSummarizer;

    #[async_trait]
    impl Summarizer for EchoSummarizer {
        async fn summarize(&self, messages: &[Message]) -> Result<String, ContextError> {
            let mut s = String::new();
            for m in messages {
                s.push_str(&Content::collapse_text(&m.content));
                s.push('|');
            }
            Ok(s)
        }
    }

    #[tokio::test]
    async fn compacts_when_history_exceeds_keep_recent() {
        let messages = vec![
            Message::user("a"),
            Message::assistant("b"),
            Message::user("c"),
            Message::assistant("d"),
            Message::user("e"),
            Message::assistant("f"),
        ];
        let config = CompactConfig {
            keep_recent_messages: 2,
            summary_prefix: "S:".into(),
        };
        let out = compact_conversation(&messages, &EchoSummarizer, &config)
            .await
            .unwrap();
        assert_eq!(out.absorbed, 4);
        assert_eq!(out.messages.len(), 3);
        assert_eq!(out.messages[0].role, Role::System);
        assert_eq!(
            Content::collapse_text(&out.messages[0].content),
            "S:a|b|c|d|"
        );
        assert_eq!(out.messages[1], Message::user("e"));
        assert_eq!(out.messages[2], Message::assistant("f"));
    }

    #[tokio::test]
    async fn no_op_when_history_is_short() {
        let messages = vec![Message::user("a"), Message::assistant("b")];
        let config = CompactConfig {
            keep_recent_messages: 4,
            ..CompactConfig::default()
        };
        let out = compact_conversation(&messages, &EchoSummarizer, &config)
            .await
            .unwrap();
        assert_eq!(out.absorbed, 0);
        assert_eq!(out.messages, messages);
    }

    #[tokio::test]
    async fn invalid_config_rejects_zero_keep() {
        let messages = vec![Message::user("a")];
        let config = CompactConfig {
            keep_recent_messages: 0,
            ..CompactConfig::default()
        };
        let err = compact_conversation(&messages, &EchoSummarizer, &config)
            .await
            .unwrap_err();
        assert!(matches!(err, ContextError::InvalidConfig(_)));
    }
}
