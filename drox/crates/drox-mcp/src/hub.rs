//! Pool de connexions MCP par serveur (chargé depuis `.mcp.json` / `mcp.json`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::Utf8Path;
use rmcp::model::ReadResourceRequestParams;
use rmcp::{Peer, RoleClient};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::config::McpJsonFile;
use crate::connect::{McpRunningClient, connect_server};
use crate::error::McpError;
use crate::names::build_mcp_tool_name;
use crate::rpc::{call_tool_json, list_resources_json, list_tools_json};

/// Métadonnées d'un tool MCP pour enregistrement dans le registre agent.
#[derive(Debug, Clone)]
pub struct McpToolStub {
    /// Clé `mcpServers` (nom config).
    pub server: String,
    /// Nom du tool côté serveur MCP.
    pub remote_name: String,
    /// Nom qualifié `mcp__<server>__<tool>`.
    pub qualified_name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}

/// Hub MCP partagé pour un run agent (connexions lazy par serveur).
pub struct McpHub {
    config: McpJsonFile,
    config_path: PathBuf,
    clients: Mutex<HashMap<String, McpRunningClient>>,
}

impl McpHub {
    /// Charge `.mcp.json` puis `mcp.json` à la racine du workspace.
    ///
    /// Renvoie `None` si aucun fichier n'existe ou si `mcpServers` est vide.
    pub async fn discover(workspace: &Utf8Path) -> Option<Arc<Self>> {
        let candidates = [workspace.join(".mcp.json"), workspace.join("mcp.json")];
        for path in candidates {
            let std = path.as_std_path();
            if !std.is_file() {
                continue;
            }
            let file = McpJsonFile::load(std).await.ok()?;
            if file.mcp_servers.is_empty() {
                continue;
            }
            return Some(Arc::new(Self {
                config: file,
                config_path: std.to_path_buf(),
                clients: Mutex::new(HashMap::new()),
            }));
        }
        None
    }

    /// Chemin du fichier de config effectivement chargé.
    #[must_use]
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Noms des serveurs déclarés dans `mcpServers`.
    #[must_use]
    pub fn server_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.config.mcp_servers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Découvre tous les tools de tous les serveurs configurés (pour stubs dynamiques).
    pub async fn discover_tool_stubs(&self) -> Vec<McpToolStub> {
        let mut out = Vec::new();
        for server in self.server_names() {
            let tools = match self.list_tools(&server).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(%server, error = %e, "MCP list_tools échoué");
                    continue;
                }
            };
            for tool_val in tools {
                if let Some(stub) = parse_tool_stub(&server, &tool_val) {
                    out.push(stub);
                }
            }
        }
        out
    }

    /// Liste les tools d'un serveur (JSON brut MCP).
    pub async fn list_tools(&self, server: &str) -> Result<Vec<Value>, McpError> {
        let peer = self.peer_for(server).await?;
        list_tools_json(&peer).await
    }

    /// Invoque un tool MCP distant.
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> Result<Value, McpError> {
        let peer = self.peer_for(server).await?;
        call_tool_json(&peer, tool_name, arguments).await
    }

    /// Liste les ressources d'un ou de tous les serveurs.
    pub async fn list_resources(&self, server: Option<&str>) -> Result<Vec<Value>, McpError> {
        let names = match server {
            Some(s) => {
                if !self.config.mcp_servers.contains_key(s) {
                    return Err(McpError::UnknownServer(s.to_string()));
                }
                vec![s.to_string()]
            }
            None => self.server_names(),
        };

        let mut out = Vec::new();
        for name in names {
            let peer = self.peer_for(&name).await?;
            let mut resources = list_resources_json(&peer).await?;
            for item in &mut resources {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("server".to_string(), json!(name));
                }
            }
            out.extend(resources);
        }
        Ok(out)
    }

    /// Lit une ressource MCP sur un serveur donné.
    pub async fn read_resource(&self, server: &str, uri: &str) -> Result<Value, McpError> {
        let peer = self.peer_for(server).await?;
        let params = ReadResourceRequestParams::new(uri);
        let result = peer.read_resource(params).await?;
        serde_json::to_value(result).map_err(Into::into)
    }

    async fn peer_for(&self, server: &str) -> Result<Peer<RoleClient>, McpError> {
        let mut guard = self.clients.lock().await;
        if let Some(client) = guard.get(server) {
            return Ok(client.peer().clone());
        }

        let entry = self.config.get_server(server)?;
        let client = connect_server(&entry).await?;
        let peer = client.peer().clone();
        guard.insert(server.to_string(), client);
        Ok(peer)
    }
}

const MAX_DESCRIPTION_LEN: usize = 2048;

fn parse_tool_stub(server: &str, tool: &Value) -> Option<McpToolStub> {
    let remote_name = tool.get("name")?.as_str()?.trim();
    if remote_name.is_empty() {
        return None;
    }
    let qualified_name = build_mcp_tool_name(server, remote_name);
    let mut description = tool
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("MCP tool")
        .to_string();
    if description.len() > MAX_DESCRIPTION_LEN {
        description.truncate(MAX_DESCRIPTION_LEN);
        description.push_str("… [truncated]");
    }
    let input_schema = tool
        .get("inputSchema")
        .or_else(|| tool.get("input_schema"))
        .cloned()
        .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
    let read_only = tool
        .get("annotations")
        .and_then(|a| {
            a.get("readOnlyHint")
                .or_else(|| a.get("read_only_hint"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(false);
    Some(McpToolStub {
        server: server.to_string(),
        remote_name: remote_name.to_string(),
        qualified_name,
        description,
        input_schema,
        read_only,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_stub_from_mcp_json() {
        let tool = json!({
            "name": "read_file",
            "description": "Read a file",
            "inputSchema": { "type": "object" },
            "annotations": { "readOnlyHint": true }
        });
        let stub = parse_tool_stub("fs", &tool).unwrap();
        assert_eq!(stub.qualified_name, "mcp__fs__read_file");
        assert!(stub.read_only);
    }
}
