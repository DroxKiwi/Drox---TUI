//! Tool `session_end` — clôture explicite du cycle de travail (mémoire longue).
//!
//! L’implémentation **locale** refuse : l’indexation (embeddings + stockage)
//! vit dans le client VS Code. Quand le client annonce `session_end` dans
//! `executableTools`, le serveur JSON-RPC remplace ce tool par un
//! [`drox_cli::jsonrpc::RemoteTool`] qui délègue via `tool/exec`.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

/// Arguments optionnels pour `session_end`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionEndInput {
    /// Indication libre pour guider la phrase d’au revoir (optionnel).
    #[serde(default)]
    pub farewell_hint: Option<String>,
}

/// Tool `session_end` (stub local).
pub struct SessionEndTool;

#[async_trait]
impl Tool for SessionEndTool {
    fn name(&self) -> &str {
        "session_end"
    }

    fn description(&self) -> &str {
        "Clôture explicitement la session de travail courante lorsque \
         l’utilisateur a signalé la fin (ex. « on s’arrête pour aujourd’hui »). \
         Après succès : une brève phrase d’au revoir / synthèse dans `answering`, \
         puis `[phase: done]`. **Dans le client Drox (VS Code)** cet appel est \
         exécuté côté IDE (archivage indexé) ; hors client délégué, le tool \
         n’est pas disponible. Format : `{ \"farewell_hint\"?: \"…\" }`."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(SessionEndInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let SessionEndInput {
            farewell_hint: _hint,
        } = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "session_end: JSON invalide ({e}). Attendu : {{ \"farewell_hint\"?: \"…\" }}."
            ))
        })?;
        Err(ToolError::invalid_args(
            "session_end: disponible uniquement lorsque le client IDE exécute cet outil \
             via tool/exec (extension VS Code Drox).",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use serde_json::json;

    #[tokio::test]
    async fn local_stub_errors() {
        let t = SessionEndTool;
        let r = t
            .execute(
                &ToolContext::new(Utf8PathBuf::from("/tmp"), false),
                json!({}),
            )
            .await;
        assert!(r.is_err());
    }
}
