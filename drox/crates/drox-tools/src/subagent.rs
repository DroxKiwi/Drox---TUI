//! Exécution de sous-agents (§2.10) — interface côté tools, implémentation dans `drox-engine`.

use async_trait::async_trait;

use crate::context::ToolContext;
use crate::error::ToolError;

/// Paramètres workspace / run pour les sous-agents.
#[derive(Debug, Clone)]
pub struct SubagentSettings {
    /// Si `false`, le tool `task` n'est pas enregistré et ne doit pas être appelé.
    pub enabled: bool,
    /// Plafond d'itérations LLM par sous-agent.
    pub max_iterations: usize,
    /// Nombre max de sous-agents en parallèle (V1 : file d'attente via sémaphore).
    pub max_concurrent: usize,
}

impl Default for SubagentSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 15,
            max_concurrent: 1,
        }
    }
}

/// Exécuteur branché par le moteur quand `SubagentSettings::enabled`.
#[async_trait]
pub trait SubagentExecutor: Send + Sync {
    /// Lance un sous-agent **Explore** (lecture seule) et renvoie le rapport final.
    async fn run_explore(
        &self,
        description: String,
        thoroughness: Option<String>,
        ctx: &ToolContext,
    ) -> Result<String, ToolError>;
}
