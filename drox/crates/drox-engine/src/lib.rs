//! `drox-engine` — boucle agent.
//!
//! Reçoit un prompt utilisateur, construit le contexte (system prompt +
//! historique + outils dispo), appelle `drox-llm` en streaming, route les
//! tool calls vers `drox-tools`, gère le bilan de tour et le streaming
//! d'événements vers le client.
//!
//! Voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 2.9 et
//! `docs/PLAN-MOTEUR-RUST.md` (sprint 1.4).
//!
//! ## Exemple
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use camino::Utf8PathBuf;
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use drox_engine::{Agent, AgentConfig, default_tool_registry};
//! use drox_llm::{LlmConfig, OllamaClient};
//! use drox_tools::ToolContext;
//! use futures::StreamExt;
//!
//! let llm = Arc::new(OllamaClient::new(LlmConfig::try_from_str(
//!     "http://localhost:11434",
//!     "llama3.2",
//! )?)?);
//! let registry = Arc::new(default_tool_registry());
//! let ctx = ToolContext::new(Utf8PathBuf::from("."), false);
//! let agent = Agent::new(llm, registry, ctx, AgentConfig::default());
//!
//! let mut stream = agent.run("lis le fichier README.md");
//! while let Some(event) = stream.next().await {
//!     println!("{event:?}");
//! }
//! # Ok(())
//! # }
//! ```

pub mod agent;
pub mod compaction;
pub mod context;
pub mod error;
pub mod event;
pub mod memory;
pub mod long_memory;
pub mod permissions;
pub mod professor;
pub mod subagent;
pub mod tool_hooks;
pub mod tool_orchestration;
pub mod tool_progress;
pub use tool_orchestration::{
    partition_tool_calls, ToolCallBatch, DEFAULT_MAX_PARALLEL_TOOL_CALLS,
};

pub use agent::{Agent, AgentConfig, AgentStream};
pub use compaction::{
    choose_live_compact_split_idx, compact_until_budget, format_compact_checkpoint, summarize_run,
    try_live_compact, CompactionConfig, CHECKPOINT_MAX_CHARS, LIVE_COMPACT_MAX_PASSES,
    LIVE_COMPACT_MAX_TAIL_RATIO, LIVE_COMPACT_TAIL_KEEP_MESSAGES,
    CompactionResult, LiveCompactReport, LIVE_COMPACT_MIN_PREFIX_TOKENS,
};
pub use memory::{MemoryRuntime, MemoryTracker, PersistedRun, persist_run};
pub use context::{ContextPolicy, SnipReport};
pub use drox_context::{
    ContextBudget, RoughTokenCounter, SnipConfig, TiktokenCounter, TokenCounter,
};
pub use drox_permissions::{
    LayeredConfig, PermissionBehavior, PermissionDecision, PermissionEngine, PermissionMode,
    PermissionTarget, Rule, RuleSet, RuleSource, RuleValue, SettingsFile, format_rule, parse_rule,
};
pub use drox_session::{
    ChatMessageRecord, DEFAULT_LISTING_LIMIT, JsonlTranscriptSink, MemorySessionEntry,
    SessionError, SessionFrontMatter, SessionListEntry, SessionUiStats, TranscriptSessionConfig,
    TranscriptSink, compute_session_path, default_sessions_dir, format_sessions_listing_for_prompt,
    list_sessions, load_memdir, load_sessions_listing, memdir_system_prefix, read_session,
    read_session_ui_stats, read_transcript, reserve_session_path, session_meta_path,
    session_ui_stats_path, slugify, display_title, read_session_meta, write_session_meta,
    SessionMeta, transcript_path, write_session, write_session_ui_stats, DroxIgnoreMatcher, WorkspaceMapStore,
};
pub use drox_tools::{
    format_skills_listing_for_prompt, load_skills_catalog, SessionNote, SessionNotesHandle,
    SubagentExecutor, SubagentSettings, ToolContext, ToolRegistry,
};
pub use subagent::{EngineSubagentExecutor, explore_tool_registry};
pub use error::EngineError;
pub use event::{AgentEvent, Phase};
pub use long_memory::{ContextChunkSummaryV1, SessionClosureV1};
pub use permissions::PermissionPolicy;
pub use drox_hooks::{load_merged as load_tool_hooks, ToolHooksConfig};

/// Registre des tools par défaut (`file_read`, `file_write`, `delete_path`, `grep`, `glob`, …).
#[must_use]
pub fn default_tool_registry() -> ToolRegistry {
    ToolRegistry::with_simple_tools()
}
