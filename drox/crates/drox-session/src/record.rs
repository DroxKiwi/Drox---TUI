//! Ligne de transcript JSONL (un message conversationnel par ligne).

use drox_types::Message;
use serde::{Deserialize, Serialize};

/// Version du schéma JSONL Drox (incrémenter en cas de rupture de compat).
pub const TRANSCRIPT_SCHEMA_VERSION: u32 = 1;

/// Une ligne de transcript : horodatage + message sérialisable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub schema_version: u32,
    /// Horodatage ISO 8601 (RFC3339).
    pub timestamp: String,
    pub message: Message,
}

impl ChatMessageRecord {
    /// Construit un enregistrement pour le message donné (horodatage = maintenant UTC).
    #[must_use]
    pub fn new(message: &Message) -> Self {
        Self {
            schema_version: TRANSCRIPT_SCHEMA_VERSION,
            timestamp: chrono::Utc::now().to_rfc3339(),
            message: message.clone(),
        }
    }
}
