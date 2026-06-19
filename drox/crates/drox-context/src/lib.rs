//! `drox-context` — gestion du contexte conversationnel.
//!
//! Trois responsabilités :
//! - **Estimation de tokens** ([`tokens`]) : trait [`TokenCounter`] et deux
//!   implémentations (`RoughTokenCounter`, `TiktokenCounter`).
//! - **Budget** ([`budget`]) : [`ContextBudget`] modélise la fenêtre du modèle
//!   et calcule les seuils (`autocompact`, `warning`, `error`,
//!   `blocking_limit`) à partir d'un usage en tokens.
//! - **Snip & compact** ([`snip`], [`compact`]) : deux stratégies de
//!   réduction de l'historique. `snip` remplace les gros résultats de tools
//!   par un placeholder. `compact` résume les anciens messages via un
//!   [`Summarizer`].
//!
//! Cette crate **ne touche pas** au filesystem et ne dépend d'aucun provider
//! LLM concret : le `Summarizer` est un trait, ce qui permet de tester la
//! compaction à vide et de brancher plus tard un `LlmClient` de `drox-llm`.

pub mod budget;
pub mod compact;
pub mod error;
pub mod microcompact;
pub mod snip;
pub mod tokens;

pub use budget::{ContextBudget, WarningState};
pub use compact::{
    CompactConfig, CompactOutcome, DEFAULT_COMPACT_INSTRUCTIONS, Summarizer, compact_conversation,
};
pub use error::ContextError;
pub use microcompact::{
    MicrocompactConfig, MicrocompactOutcome, MICROCOMPACT_CLEARED, microcompact_messages,
};
pub use snip::{SNIP_PLACEHOLDER, SnipConfig, SnipOutcome, snip_messages};
pub use tokens::{
    DEFAULT_BYTES_PER_TOKEN, IMAGE_TOKEN_SIZE, RoughTokenCounter, TiktokenCounter, TokenCounter,
};
