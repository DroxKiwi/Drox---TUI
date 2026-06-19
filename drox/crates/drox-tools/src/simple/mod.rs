//! Tools « simples » et « moyens » (sprints 1.3-1.5) + `bash` (sprint 2.2.4).
//!
//! Simple (1.3) : `file_read`, `file_write`, `delete_path`, `grep`, `glob`.
//! Moyens (1.5) : `file_edit`, `web_fetch`, `ask_user_question`, `exit_plan_mode`.
//! Bash (2.2.4) : `bash` (impl locale `sh -c` / `cmd /C` ; généralement
//! déléguée au client en mode hybride).

mod ask;
mod bash;
mod delete_path;
mod file_edit;
mod file_read;
mod file_write;
mod glob;
mod grep;
mod lsp;
mod mcp;
mod memory_list;
mod memory_read;
mod notebook_edit;
mod plan;
mod session_compact;
mod session_end;
mod session_search;
mod scope_defer;
mod session_note;
mod task;
mod skill_list;
mod skill_read;
mod git_worktree_enter;
mod git_worktree_exit;
mod copy_path;
mod course_plan_write;
mod todo_write;
mod web_fetch;
mod web_search;
mod workspace_map_note;
mod workspace_map_read;

pub use ask::{AskUserQuestionTool, CANONICAL_ASK_JSON_EXAMPLE};
pub use bash::BashTool;
pub use delete_path::DeletePathTool;
pub use file_edit::{FileEditTool, preview_file_edit_diff};
pub use file_read::FileReadTool;
pub use file_write::{FileWriteTool, preview_file_write_diff};
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use lsp::LspTool;
pub use mcp::register_mcp_tools;
pub use memory_list::MemoryListTool;
pub use memory_read::MemoryReadTool;
pub use notebook_edit::{NotebookEditTool, preview_notebook_edit_diff};
pub use plan::ExitPlanModeTool;
pub use session_compact::SessionCompactTool;
pub use session_end::SessionEndTool;
pub use session_search::SessionSearchTool;
pub use scope_defer::ScopeDeferTool;
pub use session_note::SessionNoteTool;
pub use task::TaskTool;
pub use skill_list::SkillListTool;
pub use skill_read::SkillReadTool;
pub use git_worktree_enter::GitWorktreeEnterTool;
pub use git_worktree_exit::GitWorktreeExitTool;
pub use copy_path::CopyPathTool;
pub use course_plan_write::CoursePlanWriteTool;
pub use todo_write::TodoWriteTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
pub use workspace_map_note::WorkspaceMapNoteTool;
pub use workspace_map_read::WorkspaceMapReadTool;
