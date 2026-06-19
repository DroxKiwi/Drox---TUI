//! Erreurs de la crate `drox-context`.

use thiserror::Error;

/// Erreurs renvoyées par les API publiques.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContextError {
    /// Échec d'initialisation de l'encodeur `tiktoken-rs`.
    #[error("failed to initialize tokenizer: {0}")]
    TokenizerInit(String),

    /// Erreur de sérialisation d'un bloc de contenu pour comptage.
    #[error("failed to serialize content block: {0}")]
    Serialize(#[from] serde_json::Error),

    /// Erreur remontée par un `Summarizer` (typiquement un appel LLM).
    #[error("summarizer failed: {0}")]
    Summarizer(String),

    /// Configuration de compaction invalide (ex : `keep_recent_messages == 0`).
    #[error("invalid compaction config: {0}")]
    InvalidConfig(String),
}
