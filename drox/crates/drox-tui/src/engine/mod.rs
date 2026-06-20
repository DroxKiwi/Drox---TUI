//! Pont vers `drox-engine` — bootstrap et mapping événements.

pub mod connection_library;
pub mod llm_connection;
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
pub(crate) mod llm_context;
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
pub use connection_library::{
    builtin_preset_templates, infer_provider_from_url, migrate_library_from_legacy,
    profile_to_legacy_prefs, profile_to_llm_config, profile_to_probe_config, AuthConfig,
    ConnectionLibrary, ConnectionProfile, LlmProvider, PRESET_LM_STUDIO, PRESET_OLLAMA_CLOUD,
    PRESET_OLLAMA_LOCAL, PRESET_OPENAI_COMPAT, PRESET_VLLM_OPENAI,
};
pub use preferences::{
    apply_llm_prefs_to_config, format_settings_lines, load_preferences, llm_connection_from_config,
    mark_onboarding_done, normalize_preferences, persist_from_state, preferences_path,
    record_recent_workspace, resolve_llm_startup, save_connection_library, save_llm_connection,
    save_preferences, LlmConnectionPrefs, LLM_BOOT_PLACEHOLDER_MODEL, OLLAMA_DEFAULT_SERVER,
    LlmEngineKind, TuiPreferences,
};
pub use copy_cmd::try_copy_clipboard;
pub use paste::{prepare_user_prompt, PastedTextStore};
pub use at_typeahead::{active_at_query, apply_completion, AtFileIndex, AtQuery};
pub use llm_connection::{probe_connection, probe_legacy_fields, probe_ollama};
pub use llm_context::{
    cycle_preset_index, default_max_iterations, default_num_ctx, parse_max_iterations,
    preset_index_for, preset_label, preset_count, resolve_num_ctx, CONTEXT_CUSTOM_INDEX,
    CONTEXT_PRESETS,
};
pub use add_dir_cmd::validate_workspace_path;
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
