//! Tool `git_worktree_exit` — quitte le worktree actif (garder ou supprimer).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::git_worktree::exit_worktree;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitWorktreeExitInput {
    /// `keep` laisse le worktree sur disque ; `remove` supprime worktree + branche.
    pub action: String,
    /// Obligatoire à `true` pour `remove` s'il reste des changements non commités.
    #[serde(default)]
    pub discard_changes: Option<bool>,
}

pub struct GitWorktreeExitTool;

#[async_trait]
impl Tool for GitWorktreeExitTool {
    fn name(&self) -> &str {
        "git_worktree_exit"
    }

    fn description(&self) -> &str {
        "Quitte la session worktree ouverte par git_worktree_enter. \
         action=keep conserve le dossier ; action=remove le détruit (discard_changes: \
         true requis s'il y a des fichiers/commits non intégrés). \
         Format : {\"action\": \"keep\"} ou {\"action\": \"remove\", \"discard_changes\": true}."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(GitWorktreeExitInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: GitWorktreeExitInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!("git_worktree_exit: invalid JSON ({e})"))
        })?;
        let action = args.action.trim();
        if action != "keep" && action != "remove" {
            return Err(ToolError::invalid_args(
                "git_worktree_exit: action must be \"keep\" or \"remove\"",
            ));
        }
        let discard = args.discard_changes.unwrap_or(false);
        let session = exit_worktree(&ctx.workspace_root, action, discard)
            .await
            .map_err(|e| ToolError::invalid_args(e.to_string()))?;
        let msg = if action == "keep" {
            format!(
                "Session worktree fermée. Travail conservé dans {} (branche {}).",
                session.worktree_path, session.worktree_branch
            )
        } else {
            format!(
                "Worktree {} supprimé (branche {}). Retour au dépôt principal.",
                session.worktree_path, session.worktree_branch
            )
        };
        Ok(json!({
            "action": action,
            "worktree_path": session.worktree_path.as_str(),
            "worktree_branch": session.worktree_branch,
            "main_repo_root": session.main_repo_root.as_str(),
            "message": msg,
        }))
    }
}
