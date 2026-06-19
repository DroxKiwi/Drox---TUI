//! Wrapper `Tool` qui délègue l'exécution au client connecté en JSON-RPC.
//!
//! Pour chaque tool listé dans `clientCapabilities.executableTools`, le serveur
//! remplace l'implémentation locale par un [`RemoteTool`] qui envoie une
//! requête `tool/exec` au client et attend la réponse.
//!
//! Le nom, la description et le schéma JSON sont **copiés** du tool original
//! pour que le modèle voie la même signature des deux côtés.

use std::sync::Arc;

use async_trait::async_trait;
use drox_tools::{DynTool, Tool, ToolContext, ToolError};
use serde_json::Value;
use uuid::Uuid;

use super::protocol::{ToolExecParams, ToolExecResult};
use super::server::Server;

/// Tool qui ré-émet l'appel vers le client via `tool/exec`.
pub struct RemoteTool {
    name: Arc<str>,
    description: Arc<str>,
    schema: Value,
    read_only: bool,
    server: Server,
    run_id: String,
}

impl RemoteTool {
    /// Construit un wrapper à partir d'un tool local : on récupère `name`,
    /// `description` et `input_schema`, on garde le `Server` pour pouvoir
    /// envoyer la requête `tool/exec`, et le `run_id` pour le corréler.
    #[must_use]
    pub fn wrap(server: Server, inner: &DynTool, run_id: String) -> Self {
        Self {
            name: Arc::from(inner.name()),
            description: Arc::from(inner.description()),
            schema: inner.input_schema(),
            read_only: inner.is_read_only(),
            server,
            run_id,
        }
    }
}

#[async_trait]
impl Tool for RemoteTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let params = ToolExecParams {
            run_id: self.run_id.clone(),
            call_id: Uuid::new_v4().to_string(),
            tool_name: self.name.to_string(),
            input,
            workspace: ctx.workspace_root.clone(),
            plan_mode: ctx.plan_mode,
            apply_fs_writes: ctx.apply_fs_writes,
        };

        let value = self
            .server
            .send_request("tool/exec", &params)
            .await
            .map_err(|e| {
                ToolError::remote(format!("`{}`: {} (code={})", self.name, e.message, e.code))
            })?;

        let result: ToolExecResult = serde_json::from_value(value).map_err(|e| {
            ToolError::remote(format!("`{}`: invalid `tool/exec` result: {e}", self.name))
        })?;

        if result.is_error {
            return Err(ToolError::remote(format!(
                "`{}`: client reported tool error: {}",
                self.name, result.output
            )));
        }
        Ok(result.output)
    }
}
