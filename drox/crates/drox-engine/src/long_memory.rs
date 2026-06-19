//! Mémoire longue (Sprint §2.16) — DTO sérialisés vers le client pour indexation
//! (embeddings + stockage local dans l’extension VS Code).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Résumé mécanique d’un segment d’historique évincé par compaction live (JSON v1 backlog).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextChunkSummaryV1 {
    pub schema_version: u32,
    pub id: String,
    pub workspace_fingerprint: String,
    pub transcript_session_id: String,
    pub created_at: DateTime<Utc>,
    pub compaction_seq: u32,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub summary_text: String,
    pub files_touched: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags_suggested: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_message_id: Option<String>,
}

/// Clôture de session (JSON v1 backlog) — produit côté client après `session_end`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionClosureV1 {
    pub schema_version: u32,
    pub id: String,
    pub transcript_session_id: String,
    pub closed_at: DateTime<Utc>,
    pub summary_global: String,
    pub context_chunk_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_session_slug: Option<String>,
}
