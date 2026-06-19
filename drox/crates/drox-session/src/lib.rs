//! `drox-session` — persistance des sessions.
//!
//! - Répertoire par défaut `~/.drox/sessions/` ([`paths::default_sessions_dir`]).
//! - Transcript append-only JSONL : [`ChatMessageRecord`], [`read_transcript`],
//!   [`JsonlTranscriptSink`] + trait [`TranscriptSink`] pour le moteur.
//! - Listing : [`list_sessions`].
//! - Memdir : [`load_memdir`] + [`memdir_system_prefix`] pour `MEMORY.md` /
//!   `DROX.md` à la racine du workspace.
//!
//! Voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 2.8.

pub mod drox_ignore;
pub mod error;
pub mod list;
pub mod memdir;
pub mod memory_sessions;
pub mod paths;
pub mod record;
pub mod session_meta;
pub mod transcript;
pub mod ui_stats;
pub mod workspace_map;

pub use drox_ignore::{
    DroxIgnoreMatcher, DROXIGNORE_FILENAME, DEFAULT_DROXIGNORE_TEMPLATE,
};
pub use error::SessionError;
pub use list::{SessionListEntry, list_sessions};
pub use memdir::{MemdirFiles, load_memdir, memdir_system_prefix};
pub use memory_sessions::{
    DEFAULT_LISTING_LIMIT, MemorySearchHit, MemorySessionEntry, SessionFrontMatter,
    compute_session_path, format_sessions_listing_for_prompt, load_sessions_listing,
    read_session, reserve_session_path, search_sessions, slugify, write_session,
};
pub use paths::{default_sessions_dir, session_meta_path, session_ui_stats_path, transcript_path};
pub use record::{ChatMessageRecord, TRANSCRIPT_SCHEMA_VERSION};
pub use transcript::{
    JsonlTranscriptSink, TranscriptSessionConfig, TranscriptSink, read_transcript,
};
pub use ui_stats::{SessionUiStats, read_session_ui_stats, write_session_ui_stats};
pub use session_meta::{
    display_title, read_session_meta, write_session_meta, SessionMeta,
};
pub use workspace_map::{
    WorkspaceMapStore, WorkspaceMapV1, format_workspace_map_for_prompt, initial_snapshot,
};
