//! Pont vers `drox-engine` — bootstrap et mapping événements.

mod agent_prompts;
mod bootstrap;
mod branch_cmd;
mod commands;
mod config;
mod doctor;
mod diff_cmd;
mod export_cmd;
mod files_cmd;
mod hooks;
mod init_cmd;
mod mcp;
pub(crate) mod keybindings;
pub(crate) mod keybindings_cmd;
pub(crate) mod preferences;
mod rewind;
mod skills;
pub(crate) mod notices;
pub(crate) mod paste;
pub(crate) mod image_paste;
pub(crate) mod add_dir_cmd;
pub(crate) mod at_refs;
pub(crate) mod at_typeahead;
pub(crate) mod unified_suggestions;
pub(crate) mod bash_mode;
pub(crate) mod copy_cmd;
pub(crate) mod rename_cmd;
pub(crate) mod sandbox_cmd;
pub(crate) mod status_bar;
mod statusline_cmd;
pub(crate) mod terminal_setup_cmd;
mod usage;

pub(crate) mod vim;
pub use vim::{VimComposer, VimKeyResult, VimMode};
pub use preferences::{format_settings_lines, load_preferences, mark_onboarding_done, persist_from_state, preferences_path, save_preferences, TuiPreferences};
pub use copy_cmd::try_copy_clipboard;
pub use paste::{prepare_user_prompt, PastedTextStore};
pub use at_typeahead::{active_at_query, apply_completion, AtFileIndex, AtQuery};
pub use bootstrap::{apply_agent_event, EngineRuntime, PlanModeChange};
pub use agent_prompts::{
    REVIEW_AGENT_PROMPT, SECURITY_REVIEW_AGENT_PROMPT, STATUSLINE_SETUP_PROMPT,
};
pub use mcp::{McpPanelSnapshot, McpServerLine};
pub use sandbox_cmd::format_sandbox_lines;
pub use status_bar::{format_elapsed, StatusBarSnapshot};
pub use statusline_cmd::format_statusline_lines;
pub use keybindings::{BindingAction, TuiKeybindings};
pub use commands::{compact_checkpoint_preview, CompactOutcome};
pub use init_cmd::INIT_AGENT_PROMPT;
