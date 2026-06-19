//! Estimation du nombre de tokens d'un message ou d'une conversation.
//!
//! Deux implémentations interchangeables sont fournies :
//! - [`RoughTokenCounter`] : heuristique `len / bytes_per_token` (équivalent
//!   minimal du `roughTokenCountEstimation` TS). Aucune dépendance lourde.
//! - [`TiktokenCounter`] : encode réellement le texte via `tiktoken-rs`
//!   (BPE `cl100k_base` ou `o200k_base` selon le constructeur choisi).
//!
//! Les deux implémentent le trait [`TokenCounter`], ce qui permet au reste de
//! la crate (snip, compact) de ne dépendre que du trait.

use drox_types::{Content, Message};
use serde_json::json;

use crate::error::ContextError;

/// Coût en tokens, approximatif, d'un message multimodal contenant une image.
///
/// Aligné sur la valeur prudente utilisée dans `tokenEstimation.ts` côté TS
/// (équivalent au `IMAGE_MAX_TOKEN_SIZE` du `microCompact`).
pub const IMAGE_TOKEN_SIZE: usize = 2000;

/// Rapport bytes/token par défaut pour l'estimation grossière.
pub const DEFAULT_BYTES_PER_TOKEN: usize = 4;

/// Contrat d'un compteur de tokens.
pub trait TokenCounter: Send + Sync {
    /// Compte les tokens d'une chaîne brute.
    fn count_text(&self, text: &str) -> usize;

    /// Compte les tokens d'un seul bloc de contenu (texte, `tool_use`, …).
    ///
    /// Implémentation par défaut : délègue à [`Self::count_text`] sur une
    /// représentation textuelle stable du bloc (incluant le nom du tool et la
    /// version JSON de son input, comme côté TS).
    fn count_content(&self, content: &Content) -> usize {
        match content {
            Content::Text { text } => self.count_text(text),
            Content::ToolUse { name, input, .. } => {
                let serialized = serde_json::to_string(input).unwrap_or_default();
                self.count_text(name) + self.count_text(&serialized)
            }
            Content::ToolResult {
                content, is_error, ..
            } => {
                let prefix = if *is_error { "error: " } else { "" };
                self.count_text(prefix) + self.count_text(content)
            }
            _ => self.count_text(&serde_json::to_string(content).unwrap_or_default()),
        }
    }

    /// Compte les tokens d'un message complet (tous les blocs additionnés).
    fn count_message(&self, message: &Message) -> usize {
        let mut total = 0;
        for block in &message.content {
            total += self.count_content(block);
        }
        // Petit surcoût constant par message (entête `role`, séparateurs).
        // Aligné sur la pratique tiktoken d'ajouter ~4 tokens par message
        // (voir doc OpenAI cookbook).
        total + 4
    }

    /// Compte les tokens d'une conversation entière.
    fn count_messages(&self, messages: &[Message]) -> usize {
        messages.iter().map(|m| self.count_message(m)).sum()
    }
}

/// Compteur grossier basé sur `len / bytes_per_token`.
#[derive(Debug, Clone, Copy)]
pub struct RoughTokenCounter {
    bytes_per_token: usize,
}

impl Default for RoughTokenCounter {
    fn default() -> Self {
        Self {
            bytes_per_token: DEFAULT_BYTES_PER_TOKEN,
        }
    }
}

impl RoughTokenCounter {
    /// Crée un compteur avec un ratio personnalisé (≥ 1).
    #[must_use]
    pub fn new(bytes_per_token: usize) -> Self {
        Self {
            bytes_per_token: bytes_per_token.max(1),
        }
    }
}

impl TokenCounter for RoughTokenCounter {
    fn count_text(&self, text: &str) -> usize {
        // `text.len()` en bytes (UTF-8) : volontaire, c'est aussi ce que fait
        // le TS (`content.length` en JS, qui compte les unités UTF-16, mais
        // l'ordre de grandeur visé est identique).
        text.len().div_ceil(self.bytes_per_token)
    }

    fn count_content(&self, content: &Content) -> usize {
        // L'heuristique TS attribue un coût fixe aux images et documents
        // (cf. `roughTokenCountEstimationForBlock`). On reproduit ce
        // comportement minimal : pas d'image dans `Content` actuel, mais on
        // garde la trace via une fallback sur `image:` virtuel dans la
        // sérialisation. Pour les vrais blocs, on délègue.
        let raw = self.count_text(&content_to_canonical_string(content));
        raw.max(1) // au moins 1 token par bloc non vide
    }
}

/// Compteur basé sur `tiktoken-rs`.
///
/// Charge un encodeur BPE (`cl100k_base` ou `o200k_base`). Plus précis que la
/// version grossière mais ajoute une dépendance pré-compilée non négligeable.
pub struct TiktokenCounter {
    bpe: tiktoken_rs::CoreBPE,
}

impl std::fmt::Debug for TiktokenCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TiktokenCounter").finish()
    }
}

impl TiktokenCounter {
    /// `cl100k_base` (GPT-3.5/4, Claude approximatif).
    pub fn cl100k_base() -> Result<Self, ContextError> {
        let bpe =
            tiktoken_rs::cl100k_base().map_err(|e| ContextError::TokenizerInit(e.to_string()))?;
        Ok(Self { bpe })
    }

    /// `o200k_base` (GPT-4o et plus récents).
    pub fn o200k_base() -> Result<Self, ContextError> {
        let bpe =
            tiktoken_rs::o200k_base().map_err(|e| ContextError::TokenizerInit(e.to_string()))?;
        Ok(Self { bpe })
    }
}

impl TokenCounter for TiktokenCounter {
    fn count_text(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }
}

/// Sérialise un bloc de contenu sous une forme stable, suffisante pour qu'un
/// compteur de tokens en obtienne une estimation représentative.
fn content_to_canonical_string(content: &Content) -> String {
    match content {
        Content::Text { text } => text.clone(),
        Content::ToolUse { name, input, .. } => {
            format!("{name} {input}", input = json!(input))
        }
        Content::ToolResult {
            content, is_error, ..
        } => {
            if *is_error {
                format!("error: {content}")
            } else {
                content.clone()
            }
        }
        _ => serde_json::to_string(content).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::ToolUseId;

    #[test]
    fn rough_counts_text_by_bytes_per_token() {
        let counter = RoughTokenCounter::new(4);
        assert_eq!(counter.count_text(""), 0);
        assert_eq!(counter.count_text("abcd"), 1);
        assert_eq!(counter.count_text("abcde"), 2);
        assert_eq!(counter.count_text("hello world"), 3);
    }

    #[test]
    fn rough_counts_message_includes_role_overhead() {
        let counter = RoughTokenCounter::new(4);
        let msg = Message::user("abcd"); // 1 token de contenu + 4 overhead
        assert_eq!(counter.count_message(&msg), 5);
    }

    #[test]
    fn rough_counts_tool_use_blocks() {
        let counter = RoughTokenCounter::new(4);
        let msg = Message {
            role: drox_types::Role::Assistant,
            content: vec![Content::ToolUse {
                id: ToolUseId::new(),
                name: "shell".into(),
                input: serde_json::json!({"command": "ls"}),
            }],
        };
        assert!(counter.count_message(&msg) >= 5);
    }

    #[test]
    fn rough_counts_messages_sums_each_message() {
        let counter = RoughTokenCounter::new(4);
        let msgs = vec![Message::user("abcd"), Message::assistant("efghijkl")];
        // 1 + 4 overhead, puis 2 + 4 overhead
        assert_eq!(counter.count_messages(&msgs), 5 + 6);
    }

    #[test]
    fn tiktoken_cl100k_counts_real_tokens() {
        let counter = TiktokenCounter::cl100k_base().unwrap();
        let n = counter.count_text("Hello, world!");
        assert!(n > 0 && n < 10);
    }
}
