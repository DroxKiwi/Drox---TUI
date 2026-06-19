//! Tool `workspace_map_read` — lit la carte structure workspace (§2.23).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorkspaceMapReadInput {
    /// Chemin relatif pour filtrer (préfixe). Omis = carte complète.
    #[serde(default)]
    pub path_prefix: Option<String>,
}

pub struct WorkspaceMapReadTool;

#[async_trait]
impl Tool for WorkspaceMapReadTool {
    fn name(&self) -> &str {
        "workspace_map_read"
    }

    fn description(&self) -> &str {
        "Lit la carte structure persistante du workspace (`.drox/workspace-map.json`) : \
         zones, rôles, pivots. Utilise-la avant un `glob` large si le system prompt en \
         mentionne une version fresh."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(WorkspaceMapReadInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: WorkspaceMapReadInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "workspace_map_read: JSON invalide ({e}). Attendu : {{\"path_prefix\": \"drox/\"}} (optionnel)."
            ))
        })?;
        let store = ctx.workspace_map.as_ref().ok_or_else(|| {
            ToolError::invalid_args(
                "workspace_map_read: tool unavailable in this context (no workspace map wired)",
            )
        })?;
        let map = store.snapshot();
        let prefix = args
            .path_prefix
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_string());
        let nodes: Vec<_> = if let Some(ref p) = prefix {
            map.nodes
                .iter()
                .filter(|n| n.path == *p || n.path.starts_with(&format!("{p}/")))
                .cloned()
                .collect()
        } else {
            map.nodes.clone()
        };
        Ok(json!({
            "version": map.version,
            "stale": map.stale,
            "updated_at": map.updated_at,
            "node_count": nodes.len(),
            "nodes": nodes,
        }))
    }
}
