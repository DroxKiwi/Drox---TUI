//! Réduction (« snip ») des gros résultats de tools pour libérer du contexte.
//!
//! Stratégie minimale, alignée sur l'esprit du `microCompact` TS :
//! - on parcourt les messages dans l'ordre **chronologique** ;
//! - pour chaque bloc `ToolResult` dont l'estimation de tokens dépasse
//!   [`SnipConfig::min_tokens`], on remplace son contenu par un placeholder
//!   court ;
//! - on **préserve** les `keep_recent_results` derniers résultats de tools
//!   (on n'a pas envie de tronquer ce que le modèle vient juste de lire).
//!
//! Cette approche est volontairement plus simple que le TS (pas de bucket par
//! type d'outil, pas d'attachements pinés). Elle suffit pour le moteur Rust
//! en début de Phase 1 et peut être étendue plus tard.

use drox_types::{Content, Message};

use crate::tokens::TokenCounter;

/// Texte qui remplace le contenu d'un `ToolResult` snippé.
pub const SNIP_PLACEHOLDER: &str = "[Tool output truncated for context efficiency]";

/// Configuration du snip.
#[derive(Debug, Clone)]
pub struct SnipConfig {
    /// Taille minimale (en tokens estimés) pour qu'un `ToolResult` soit snippé.
    pub min_tokens: usize,
    /// Nombre de `ToolResult` les plus récents à conserver intacts.
    pub keep_recent_results: usize,
    /// Placeholder substitué (par défaut [`SNIP_PLACEHOLDER`]).
    pub placeholder: String,
}

impl Default for SnipConfig {
    fn default() -> Self {
        Self {
            min_tokens: 1_000,
            keep_recent_results: 3,
            placeholder: SNIP_PLACEHOLDER.into(),
        }
    }
}

/// Résultat d'une passe de snip.
#[derive(Debug, Clone)]
pub struct SnipOutcome {
    /// Messages potentiellement modifiés.
    pub messages: Vec<Message>,
    /// Estimation de tokens libérés (sommes des deltas avant / après).
    pub tokens_freed: usize,
    /// Nombre de blocs `ToolResult` effectivement remplacés.
    pub blocks_snipped: usize,
}

/// Applique un snip sur la conversation.
pub fn snip_messages<C: TokenCounter + ?Sized>(
    messages: &[Message],
    counter: &C,
    config: &SnipConfig,
) -> SnipOutcome {
    // Indexe les blocs `ToolResult` (message_idx, content_idx) par ordre.
    let mut tool_result_positions: Vec<(usize, usize)> = Vec::new();
    for (mi, m) in messages.iter().enumerate() {
        for (ci, c) in m.content.iter().enumerate() {
            if matches!(c, Content::ToolResult { .. }) {
                tool_result_positions.push((mi, ci));
            }
        }
    }

    // Les `keep_recent_results` derniers sont préservés.
    let keep_from = tool_result_positions
        .len()
        .saturating_sub(config.keep_recent_results);
    let snippable: std::collections::HashSet<(usize, usize)> =
        tool_result_positions.into_iter().take(keep_from).collect();

    let mut out: Vec<Message> = messages.to_vec();
    let mut tokens_freed: usize = 0;
    let mut blocks_snipped: usize = 0;

    for (mi, msg) in out.iter_mut().enumerate() {
        for (ci, block) in msg.content.iter_mut().enumerate() {
            if !snippable.contains(&(mi, ci)) {
                continue;
            }
            let Content::ToolResult {
                tool_use_id,
                content,
                is_error,
            } = block
            else {
                continue;
            };

            let before = counter.count_text(content);
            if before < config.min_tokens {
                continue;
            }
            let after = counter.count_text(&config.placeholder);
            tokens_freed = tokens_freed.saturating_add(before.saturating_sub(after));
            blocks_snipped += 1;
            *block = Content::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: config.placeholder.clone(),
                is_error: *is_error,
            };
        }
    }

    SnipOutcome {
        messages: out,
        tokens_freed,
        blocks_snipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::RoughTokenCounter;
    use drox_types::{Role, ToolUseId};

    fn tool_result_msg(content: &str) -> Message {
        Message {
            role: Role::Tool,
            content: vec![Content::ToolResult {
                tool_use_id: ToolUseId::new(),
                content: content.into(),
                is_error: false,
            }],
        }
    }

    #[test]
    fn snips_old_large_results_keeps_recent() {
        let counter = RoughTokenCounter::new(4);
        let big = "x".repeat(8_000); // ~2000 tokens
        let messages = vec![
            tool_result_msg(&big),
            tool_result_msg(&big),
            tool_result_msg(&big),
            tool_result_msg(&big),
            tool_result_msg(&big),
        ];
        let config = SnipConfig {
            min_tokens: 100,
            keep_recent_results: 2,
            placeholder: "[snip]".into(),
        };
        let out = snip_messages(&messages, &counter, &config);
        assert_eq!(out.blocks_snipped, 3);
        assert!(out.tokens_freed > 0);

        let snipped_text = match &out.messages[0].content[0] {
            Content::ToolResult { content, .. } => content.clone(),
            _ => panic!("expected ToolResult"),
        };
        assert_eq!(snipped_text, "[snip]");

        let preserved = match &out.messages[4].content[0] {
            Content::ToolResult { content, .. } => content.clone(),
            _ => panic!("expected ToolResult"),
        };
        assert_eq!(preserved, big);
    }

    #[test]
    fn does_not_snip_small_results() {
        let counter = RoughTokenCounter::new(4);
        let messages = vec![
            tool_result_msg("tiny"),
            tool_result_msg("tiny"),
            tool_result_msg("tiny"),
        ];
        let config = SnipConfig::default();
        let out = snip_messages(&messages, &counter, &config);
        assert_eq!(out.blocks_snipped, 0);
        assert_eq!(out.tokens_freed, 0);
        assert_eq!(out.messages, messages);
    }

    #[test]
    fn ignores_non_tool_result_messages() {
        let counter = RoughTokenCounter::new(4);
        let messages = vec![
            Message::user("question"),
            Message::assistant("réponse longue ".repeat(2_000)),
        ];
        let config = SnipConfig::default();
        let out = snip_messages(&messages, &counter, &config);
        assert_eq!(out.blocks_snipped, 0);
        assert_eq!(out.messages, messages);
    }
}
