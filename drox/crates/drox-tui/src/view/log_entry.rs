//! Entrées du fil de discussion.

use std::collections::HashSet;

use drox_engine::Phase;
use drox_types::ToolUseId;
use serde_json::Value;

use super::api_error;
use super::format_bash_finish_lines;
use super::bash_output::format_bash_mode_finish_lines;
use super::extra_output::{
    format_exit_plan_mode_finish, format_exit_plan_mode_start, format_git_worktree_enter_finish,
    format_git_worktree_enter_start, format_git_worktree_exit_finish, format_git_worktree_exit_start,
    format_lsp_finish, format_lsp_start, format_skill_list_finish, format_skill_read_finish,
    format_task_finish, format_task_start,
};
use super::tool_output::{
    format_copy_path_finish, format_delete_path_finish, format_file_edit_finish,
    format_file_read_finish, format_file_write_finish, format_glob_finish, format_grep_finish,
    format_notebook_edit_finish, format_web_fetch_finish, format_web_search_finish,
};
use super::mcp_output::{format_mcp_finish, format_mcp_start, is_mcp_tool};

/// Une ligne du journal de session affiché dans le TUI.
#[derive(Debug, Clone)]
pub enum LogEntry {
    User {
        text: String,
    },
    System {
        text: String,
    },
    PhaseOpen {
        phase: Phase,
    },
    PhaseLine {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolStart {
        id: ToolUseId,
        name: String,
        arguments: Value,
    },
    ToolFinish {
        id: ToolUseId,
        name: String,
        output: Value,
        is_error: bool,
    },
    ContextSnip {
        tokens_freed: usize,
        blocks_snipped: usize,
    },
    ContextCompacted {
        tokens_before: usize,
        tokens_after: usize,
        messages_removed: usize,
    },
    MemoryPersisted {
        slug: String,
        objective: String,
    },
    RunObjective {
        text: String,
    },
    /// Mode plan activé (`/plan` ou `--plan`).
    PlanActivated {
        objective: Option<String>,
    },
    /// Contenu de `.drox/plan.md` affiché via `/plan`.
    PlanDocument {
        path: String,
        body: String,
        truncated: bool,
    },
    /// Décision humaine sur `exit_plan_mode`.
    PlanApproval {
        accepted: bool,
        response: String,
    },
    Error {
        text: String,
    },
    /// Commande `!` saisie par l'utilisateur (mode bash intégré).
    BashModeInput {
        command: String,
    },
    /// Sortie du mode bash intégré.
    BashModeOutput {
        id: ToolUseId,
        output: Value,
        is_error: bool,
    },
    /// Run agent interrompu (Esc / Ctrl+C).
    RunCancelled {
        reason: String,
    },
    /// Erreur API / LLM structurée.
    ApiError(crate::view::api_error::ApiErrorView),
}

impl LogEntry {
    #[must_use]
    pub fn display_lines(&self, expanded_tools: &HashSet<ToolUseId>) -> Vec<String> {
        match self {
            Self::User { text } => vec![format!("▸ {text}")],
            Self::System { text } => vec![format!("· {text}")],
            Self::PhaseOpen { phase } => vec![format!("── [{}] ──", phase.as_marker())],
            Self::PhaseLine { text } => text.lines().map(|l| format!("  {l}")).collect(),
            Self::Assistant { text } => {
                if text.trim().is_empty() {
                    vec![]
                } else {
                    vec![format!("◂ {text}")]
                }
            }
            Self::ToolStart { name, id, arguments } => {
                if name == "bash" {
                    let cmd = arguments
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    return vec![format!("▸ bash ({id}) $ {cmd}")];
                }
                if name == "web_fetch" {
                    let url = arguments
                        .get("url")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    return vec![format!("▸ web_fetch ({id}) {url}")];
                }
                if name == "web_search" {
                    let query = arguments
                        .get("query")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    return vec![format!("▸ web_search ({id}) «{query}»")];
                }
                if is_mcp_tool(name) {
                    return format_mcp_start(name, id, arguments);
                }
                if name == "notebook_edit" {
                    let path = arguments
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    let n = arguments
                        .get("edits")
                        .and_then(|v| v.as_array())
                        .map(std::vec::Vec::len)
                        .unwrap_or(0);
                    return vec![format!("▸ notebook_edit ({id}) {path} ({n} edit(s))")];
                }
                if name == "file_edit" {
                    let path = arguments
                        .get("file_path")
                        .or_else(|| arguments.get("path"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    let n = arguments
                        .get("edits")
                        .and_then(|v| v.as_array())
                        .map(std::vec::Vec::len)
                        .unwrap_or(1);
                    return vec![format!("▸ file_edit ({id}) {path} ({n} edit(s))")];
                }
                if name == "file_write" {
                    let path = arguments
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    return vec![format!("▸ file_write ({id}) {path}")];
                }
                if name == "delete_path" {
                    let path = arguments
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    let rec = arguments
                        .get("recursive")
                        .and_then(serde_json::Value::as_bool)
                        .filter(|r| *r)
                        .map(|_| " récursif")
                        .unwrap_or("");
                    return vec![format!("▸ delete_path ({id}) {path}{rec}")];
                }
                if name == "copy_path" {
                    let source = arguments
                        .get("source")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    let dest = arguments
                        .get("destination")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    return vec![format!("▸ copy_path ({id}) {source} → {dest}")];
                }
                if name == "exit_plan_mode" {
                    return format_exit_plan_mode_start(id, arguments);
                }
                if name == "lsp" {
                    return format_lsp_start(id, arguments);
                }
                if name == "skill_list" {
                    return vec![format!("▸ skill_list ({id})")];
                }
                if name == "skill_read" {
                    let skill = arguments
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    return vec![format!("▸ skill_read ({id}) {skill}")];
                }
                if name == "git_worktree_enter" {
                    return format_git_worktree_enter_start(id, arguments);
                }
                if name == "git_worktree_exit" {
                    return format_git_worktree_exit_start(id, arguments);
                }
                if name == "task" {
                    return format_task_start(id, arguments);
                }
                if name == "todo_write" {
                    let n = arguments
                        .get("todos")
                        .and_then(|v| v.as_array())
                        .map(std::vec::Vec::len)
                        .unwrap_or(0);
                    return vec![format!("▸ todo_write ({id}) {n} tâche(s)")];
                }
                let args = serde_json::to_string(arguments).unwrap_or_else(|_| "{}".into());
                let short = if args.len() > 120 {
                    format!("{}…", &args[..120])
                } else {
                    args
                };
                vec![format!("▸ tool {name} ({id}) {short}")]
            }
            Self::ToolFinish {
                name,
                id,
                output,
                is_error,
            } => {
                if name == "bash" {
                    format_bash_finish_lines(id, output, *is_error, expanded_tools.contains(id))
                } else if name == "file_read" {
                    format_file_read_finish(id, output, *is_error, expanded_tools.contains(id))
                } else if name == "file_edit" {
                    format_file_edit_finish(id, output, *is_error)
                } else if name == "file_write" {
                    format_file_write_finish(id, output, *is_error)
                } else if name == "delete_path" {
                    format_delete_path_finish(id, output, *is_error)
                } else if name == "copy_path" {
                    format_copy_path_finish(id, output, *is_error)
                } else if name == "grep" {
                    format_grep_finish(id, output, *is_error, expanded_tools.contains(id))
                } else if name == "glob" {
                    format_glob_finish(id, output, *is_error)
                } else if name == "web_fetch" {
                    format_web_fetch_finish(id, output, *is_error, expanded_tools.contains(id))
                } else if name == "web_search" {
                    format_web_search_finish(id, output, *is_error)
                } else if is_mcp_tool(name) {
                    format_mcp_finish(name, id, output, *is_error)
                } else if name == "notebook_edit" {
                    format_notebook_edit_finish(id, output, *is_error)
                } else if name == "exit_plan_mode" {
                    format_exit_plan_mode_finish(id, output, *is_error)
                } else if name == "lsp" {
                    format_lsp_finish(id, output, *is_error)
                } else if name == "skill_list" {
                    format_skill_list_finish(id, output, *is_error)
                } else if name == "skill_read" {
                    format_skill_read_finish(id, output, *is_error)
                } else if name == "git_worktree_enter" {
                    format_git_worktree_enter_finish(id, output, *is_error)
                } else if name == "git_worktree_exit" {
                    format_git_worktree_exit_finish(id, output, *is_error)
                } else if name == "task" {
                    format_task_finish(id, output, *is_error)
                } else if name == "todo_write" {
                    format_todo_write_finish(id, output, *is_error)
                } else if name == "course_plan_write" {
                    format_course_plan_write_finish(id, output, *is_error)
                } else {
                    format_tool_finish(name, id, output, *is_error)
                }
            }
            Self::ContextSnip {
                tokens_freed,
                blocks_snipped,
            } => vec![format!(
                "· contexte snip (~{tokens_freed} tok, {blocks_snipped} blocs)"
            )],
            Self::ContextCompacted {
                tokens_before,
                tokens_after,
                messages_removed,
            } => vec![format!(
                "· compaction live {tokens_before}→{tokens_after} tok ({messages_removed} msgs)"
            )],
            Self::MemoryPersisted { slug, objective } => vec![format!("· session archivée [{slug}] {objective}")],
            Self::RunObjective { text } => vec![format!("· objectif : {text}")],
            Self::PlanActivated { objective } => {
                let mut lines = vec!["── [PLAN] mode activé ──".to_string()];
                if let Some(obj) = objective {
                    if !obj.trim().is_empty() {
                        lines.push(format!("  objectif : {obj}"));
                    }
                }
                lines
            }
            Self::PlanDocument {
                path,
                body,
                truncated,
            } => {
                let mut lines = vec![format!("── [PLAN] {path} ──")];
                let max = crate::view::extra_output::PLAN_DOCUMENT_MAX_LINES;
                let body_lines: Vec<&str> = body.lines().collect();
                let show_truncated = *truncated || body_lines.len() > max;
                let take = if show_truncated {
                    max.min(body_lines.len())
                } else {
                    body_lines.len()
                };
                for line in body_lines.iter().take(take) {
                    lines.push(format!("  {line}"));
                }
                if show_truncated {
                    let hidden = body_lines.len().saturating_sub(take);
                    lines.push(format!(
                        "  … +{} lignes (e pour parcourir)",
                        hidden.max(1)
                    ));
                }
                lines
            }
            Self::PlanApproval { accepted, response } => {
                let tag = if *accepted {
                    "approuvé — exécution"
                } else {
                    "refusé — reste en plan"
                };
                vec![
                    format!("── [PLAN] {tag} ──"),
                    format!("  réponse : {response}"),
                ]
            }
            Self::Error { text } => vec![format!("✗ {text}")],
            Self::BashModeInput { command } => vec![format!("▸ ! {command}")],
            Self::BashModeOutput { id, output, is_error } => {
                format_bash_mode_finish_lines(id, output, *is_error, expanded_tools.contains(id))
            }
            Self::RunCancelled { reason } => vec![format!("⊘ run annulé — {reason}")],
            Self::ApiError(view) => api_error::render_api_error(view)
                .into_iter()
                .map(|l| l.to_string())
                .collect(),
        }
    }
}

fn format_todo_write_finish(id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        let body = serde_json::to_string(output).unwrap_or_else(|_| "{}".into());
        return vec![format!("◂ todo_write ({id}) [erreur] {body}")];
    }
    let summary = output
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("todos mis à jour");
    vec![format!("· todo_write ({id}) {summary}")]
}

fn format_course_plan_write_finish(
    id: &ToolUseId,
    output: &Value,
    is_error: bool,
) -> Vec<String> {
    if is_error {
        let body = serde_json::to_string(output).unwrap_or_else(|_| "{}".into());
        return vec![format!("◂ course_plan_write ({id}) [erreur] {body}")];
    }
    let summary = output
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("plan de cours mis à jour");
    vec![format!("· course_plan_write ({id}) {summary}")]
}

fn format_tool_finish(name: &str, id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    let tag = if is_error { "erreur" } else { "ok" };
    let body = serde_json::to_string(output).unwrap_or_else(|_| "{}".into());
    let short = if body.len() > 200 {
        format!("{}…", &body[..200])
    } else {
        body
    };
    vec![format!("◂ {name} ({id}) [{tag}] {short}")]
}
