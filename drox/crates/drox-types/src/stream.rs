//! Événements de streaming émis par un client LLM.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::ToolUseId;

/// Raison d'arrêt d'une génération.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StopReason {
    /// Fin naturelle du tour assistant.
    EndTurn,
    /// Limite de tokens atteinte.
    MaxTokens,
    /// Séquence d'arrêt rencontrée.
    StopSequence,
    /// L'assistant a demandé l'exécution d'un tool.
    ToolUse,
    /// Arrêt suite à une erreur côté provider.
    Error,
}

/// Comptage de tokens (input/output) d'une requête.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Événement émis pendant un appel streaming au LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum StreamEvent {
    /// Début du tour assistant.
    Start,
    /// Token(s) de texte produits par l'assistant.
    TextDelta { text: String },
    /// Fragment du canal « pensée » natif du fournisseur (ex. Ollama
    /// `message.thinking` pour les modèles *thinking*). Distinct du texte
    /// assistant affiché dans le protocole Drox (`[phase: …]`).
    ThinkingDelta { text: String },
    /// Demande d'exécution d'un tool par l'assistant (atomique : Ollama
    /// n'émet pas de delta partiel sur `tool_calls`).
    ToolCall {
        id: ToolUseId,
        name: String,
        arguments: Value,
    },
    /// Fin du tour assistant.
    Stop { reason: StopReason, usage: Usage },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_event_serde_round_trip() {
        let event = StreamEvent::TextDelta {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn stop_event_serializes_with_kind_tag() {
        let event = StreamEvent::Stop {
            reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 10,
                output_tokens: 20,
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "stop");
        assert_eq!(json["reason"], "end_turn");
        assert_eq!(json["usage"]["input_tokens"], 10);
    }
}
