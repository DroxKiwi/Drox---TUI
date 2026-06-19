//! Tool `session_search` — recherche dans la mémoire longue indexée (RAG léger).
//!
//! Les vecteurs et le stockage vivent dans l’extension VS Code. L’impl
//! **locale** refuse ; avec `executableTools` côté client, un
//! [`RemoteTool`] exécute `tool/exec`.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

/// Arguments de `session_search`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionSearchInput {
    /// Requête en langage naturel (souvent la question utilisateur ou des mots-clés).
    pub query: String,
    /// Nombre max de résultats (défaut côté client si absent).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Tool `session_search` (stub local).
pub struct SessionSearchTool;

#[async_trait]
impl Tool for SessionSearchTool {
    fn name(&self) -> &str {
        "session_search"
    }

    fn description(&self) -> &str {
        "Recherche dans la **mémoire longue** du workspace (résumés de compactions \
         indexés + clôtures de session). Retourne les extraits les plus pertinents \
         pour enrichir le contexte. **Exécution** : client IDE (extension Drox) ; \
         hors client délégué, indisponible. Format : `{ \"query\": \"…\", \"limit\"?: N }`."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(SessionSearchInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let SessionSearchInput {
            query,
            limit: _limit,
        } = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "session_search: JSON invalide ({e}). Attendu : {{ \"query\": \"…\", \"limit\"?: N }}."
            ))
        })?;
        if query.trim().is_empty() {
            return Err(ToolError::invalid_args("session_search: `query` ne peut pas être vide."));
        }
        Err(ToolError::invalid_args(
            "session_search: disponible uniquement lorsque le client IDE exécute cet outil \
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
    async fn rejects_empty_query() {
        let t = SessionSearchTool;
        let r = t
            .execute(
                &ToolContext::new(Utf8PathBuf::from("/tmp"), false),
                json!({ "query": "  " }),
            )
            .await;
        assert!(r.is_err());
    }
}
