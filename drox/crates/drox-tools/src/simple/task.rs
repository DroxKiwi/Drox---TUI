//! Tool `task` — délègue une exploration en lecture seule à un sous-agent (§2.10).
//!
//! Désactivé par défaut (`drox.subagents.enabled` / `subagentsEnabled` JSON-RPC).
//! V1 : seul `subagent_type: explore` est supporté.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskInput {
    /// Description de la tâche de recherche / exploration.
    pub description: String,
    /// Type de sous-agent. V1 : uniquement `explore`.
    #[serde(default = "default_subagent_type")]
    pub subagent_type: String,
    /// Niveau d'exploration : `quick` | `medium` | `very thorough`.
    #[serde(default)]
    pub thoroughness: Option<String>,
}

fn default_subagent_type() -> String {
    "explore".to_string()
}

pub struct TaskTool;

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Délègue une exploration **lecture seule** à un sous-agent (grep, glob, file_read, lsp). \
         Réservé aux recherches larges ou parallélisables — pas pour une lecture ciblée d'un \
         fichier connu (utilise `file_read`). **Désactivé** si les sous-agents ne sont pas \
         activés dans les paramètres (`drox.subagents.enabled`). V1 : `subagent_type` = `explore`."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(TaskInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let settings = ctx
            .subagent_settings
            .as_ref()
            .cloned()
            .unwrap_or_default();
        if !settings.enabled {
            return Err(ToolError::invalid_args(
                "Sous-agents désactivés. Activez `drox.subagents.enabled` dans les paramètres \
                 VS Code pour utiliser `task`.",
            ));
        }
        let executor = ctx.subagent_executor.as_ref().ok_or_else(|| {
            ToolError::invalid_args(
                "Sous-agents non configurés pour ce run (exécuteur manquant côté moteur).",
            )
        })?;
        let args: TaskInput = serde_json::from_value(input)?;
        if args.description.trim().is_empty() {
            return Err(ToolError::invalid_args("description must not be empty"));
        }
        let kind = args.subagent_type.trim().to_ascii_lowercase();
        if kind != "explore" {
            return Err(ToolError::invalid_args(format!(
                "subagent_type `{kind}` inconnu — V1 supporte uniquement `explore`"
            )));
        }
        let report = executor
            .run_explore(args.description, args.thoroughness, ctx)
            .await?;
        Ok(json!({
            "subagent_type": "explore",
            "report": report,
        }))
    }
}
