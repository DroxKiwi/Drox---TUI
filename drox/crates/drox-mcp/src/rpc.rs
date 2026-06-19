//! Appels RPC de haut niveau (`list_tools`, `call_tool`, resources) vers JSON pour l'intégration Drox.

use rmcp::model::CallToolRequestParams;
use rmcp::{Peer, RoleClient};
use serde_json::Value;

use crate::error::McpError;

/// Liste tous les tools du serveur distant et les sérialise en JSON.
pub async fn list_tools_json(peer: &Peer<RoleClient>) -> Result<Vec<Value>, McpError> {
    let tools = peer.list_all_tools().await?;
    tools
        .into_iter()
        .map(|t| serde_json::to_value(t).map_err(McpError::from))
        .collect()
}

/// Invoque un tool MCP et renvoie le résultat en JSON.
pub async fn call_tool_json(
    peer: &Peer<RoleClient>,
    tool_name: &str,
    arguments: Option<Value>,
) -> Result<Value, McpError> {
    let args = match arguments {
        None | Some(Value::Null) => None,
        Some(Value::Object(m)) => Some(m),
        Some(other) => {
            return Err(McpError::config(format!(
                "arguments d'outil MCP doivent être un objet JSON, reçu: {other}"
            )));
        }
    };

    let mut params = CallToolRequestParams::new(tool_name.to_string());
    if let Some(m) = args {
        params = params.with_arguments(m);
    }

    let result = peer.call_tool(params).await?;
    serde_json::to_value(result).map_err(Into::into)
}

/// Variante pratique : arguments déjà sous forme d'objet `serde_json`.
pub async fn call_tool_object(
    peer: &Peer<RoleClient>,
    tool_name: &str,
    arguments: serde_json::Map<String, Value>,
) -> Result<Value, McpError> {
    call_tool_json(peer, tool_name, Some(Value::Object(arguments))).await
}

/// Liste toutes les ressources du serveur et les sérialise en JSON.
pub async fn list_resources_json(peer: &Peer<RoleClient>) -> Result<Vec<Value>, McpError> {
    let resources = peer.list_all_resources().await?;
    resources
        .into_iter()
        .map(|r| serde_json::to_value(r).map_err(McpError::from))
        .collect()
}

/// Variante pratique : renvoie un tableau vide si le serveur ne supporte pas les ressources.
pub async fn list_resources_json_or_empty(peer: &Peer<RoleClient>) -> Vec<Value> {
    match list_resources_json(peer).await {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, "list_resources ignoré (serveur sans capability resources?)");
            Vec::new()
        }
    }
}
