//! Trait `Tool` et types associés.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::context::ToolContext;
use crate::error::ToolError;

/// Tool exécutable de façon asynchrone.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Identifiant unique du tool (ex: `file_read`, `mcp__server__tool`).
    fn name(&self) -> &str;

    /// Description courte pour le prompt système / le catalogue.
    fn description(&self) -> &str;

    /// Schéma JSON des arguments d'entrée (JSON Schema).
    fn input_schema(&self) -> Value;

    /// `true` si le tool ne modifie pas l'environnement (hint permissions).
    fn is_read_only(&self) -> bool {
        false
    }

    /// `true` si plusieurs invocations peuvent tourner en parallèle dans un même tour.
    /// Par défaut aligné sur [`Self::is_read_only`] (cf. leak `isConcurrencySafe`).
    fn is_concurrency_safe(&self) -> bool {
        self.is_read_only()
    }

    /// Exécute le tool avec le contexte session et l'entrée JSON brute.
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError>;
}

/// Boîte type-erased pour stocker les tools dans un registre.
pub type DynTool = Arc<dyn Tool>;
