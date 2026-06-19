//! Tool `workspace_map_note` — annotation sémantique sur la carte workspace (§2.23).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorkspaceMapNoteInput {
    /// Chemin relatif du dossier ou fichier (vide ou `.` = racine).
    #[serde(default)]
    pub path: Option<String>,
    /// Rôle en une ligne (« moteur Rust », « UI chat », …).
    pub summary: String,
}

pub struct WorkspaceMapNoteTool;

#[async_trait]
impl Tool for WorkspaceMapNoteTool {
    fn name(&self) -> &str {
        "workspace_map_note"
    }

    fn description(&self) -> &str {
        "Ajoute ou met à jour une **note sémantique** sur la carte workspace (zone, rôle, \
         pivot). À utiliser après exploration pour figer « à quoi sert ce dossier » sans \
         relister tout le repo."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(WorkspaceMapNoteInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: WorkspaceMapNoteInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "workspace_map_note: JSON invalide ({e}). Attendu : \
                 {{\"path\": \"drox/\", \"summary\": \"moteur agent\"}}."
            ))
        })?;
        let store = ctx.workspace_map.as_ref().ok_or_else(|| {
            ToolError::invalid_args(
                "workspace_map_note: tool unavailable in this context (no workspace map wired)",
            )
        })?;
        let path = args.path.as_deref().map(|p| {
            let t = p.trim();
            if t == "." { "" } else { t }
        });
        let node = store
            .note(path, args.summary)
            .map_err(ToolError::invalid_args)?;
        store.save_if_dirty().await;
        Ok(json!({
            "ok": true,
            "path": node.path,
            "summary": node.summary,
        }))
    }
}
