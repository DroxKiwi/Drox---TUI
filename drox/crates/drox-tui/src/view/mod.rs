//! Modèle de vue (fil, streaming).

pub mod ansi;
pub mod api_error;
pub mod bash_output;
pub mod bash_progress;
pub mod extra_output;
pub mod log_cache;
pub mod log_entry;
pub mod link;
pub mod markdown;
pub mod message_router;
pub mod user_message;
pub mod mcp_output;
pub mod permission_preview;
pub mod render;
pub mod run_spinner;
pub mod spinner;
pub mod system_message;
pub mod syntax;
pub mod tool_output;
pub mod lines_viewer;
pub mod glob_viewer;
pub mod grep_viewer;
pub mod scroll_viewer;
pub mod web_fetch_viewer;
pub mod web_search_viewer;
pub mod bash_viewer;
pub mod file_read_viewer;
pub mod history_search;
pub mod hook_progress;
pub mod transcript_replay;
pub mod transcript_search;

pub use bash_output::{bash_kind_label, format_bash_finish_lines};
pub use bash_progress::{progress_lines, ActiveBashRun};
pub use scroll_viewer::ScrollViewerState;
pub use hook_progress::{progress_lines as hook_progress_lines, ActiveHookProgress};
pub use log_entry::LogEntry;
pub use permission_preview::{
    bash_kind_color, compute_permission_preview, parse_plan_approval_preview,
    preview_body_line_count, PermissionPreview, PermissionPreviewBody,
};
pub use history_search::{find_history_match_indices, HistorySearchState};
pub use transcript_replay::messages_to_log_entries;
pub use transcript_search::{
    find_matching_line_indices, highlight_line, scroll_to_line, TranscriptSearchState,
};
