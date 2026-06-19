//! Schéma de fichier `.mcp.json` (sous-ensemble aligné sur le moteur TS).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::env_expand::{expand_env_in_map, expand_env_in_string};
use crate::error::McpError;

/// Racine d'un fichier `mcp.json` / `.mcp.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct McpJsonFile {
    /// Table des serveurs nommés (même clé que côté TypeScript).
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, Value>,
}

/// Spécification d'un serveur **stdio** (commande locale).
#[derive(Debug, Clone, Deserialize)]
pub struct McpStdioSpec {
    #[serde(rename = "type", default)]
    pub transport_type: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Spécification d'un serveur **HTTP** ou **SSE** distant (transport streamable HTTP rmcp).
#[derive(Debug, Clone, Deserialize)]
pub struct McpRemoteSpec {
    /// `http`, `sse`, etc. Omis → traité comme `http` (serveurs minimalistes).
    #[serde(rename = "type", default)]
    pub transport_type: Option<String>,
    pub url: String,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

impl McpRemoteSpec {
    #[must_use]
    pub fn transport_type_normalized(&self) -> String {
        self.transport_type
            .clone()
            .unwrap_or_else(|| "http".to_string())
            .to_ascii_lowercase()
    }
}

/// Entrée serveur après résolution du format.
#[derive(Debug, Clone)]
pub enum McpServerEntry {
    Stdio(McpStdioSpec),
    Remote(McpRemoteSpec),
}

impl McpJsonFile {
    /// Parse JSON brut puis valide la racine `mcpServers`.
    pub fn from_json_slice(data: &[u8]) -> Result<Self, McpError> {
        serde_json::from_slice(data).map_err(Into::into)
    }

    /// Lit un fichier UTF-8 depuis le disque.
    pub async fn load(path: &Path) -> Result<Self, McpError> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| McpError::config(format!("lecture {}: {e}", path.display())))?;
        Self::from_json_slice(&bytes)
    }

    /// Récupère et normalise la config d'un serveur par son nom.
    pub fn get_server(&self, name: &str) -> Result<McpServerEntry, McpError> {
        let raw = self
            .mcp_servers
            .get(name)
            .ok_or_else(|| McpError::UnknownServer(name.to_string()))?;
        parse_server_entry(raw)
    }
}

/// Interprète une valeur `mcpServers.<name>` (stdio vs remote).
pub fn parse_server_entry(raw: &Value) -> Result<McpServerEntry, McpError> {
    let has_url = raw
        .get("url")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty());
    let has_command = raw
        .get("command")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty());

    if has_url && !has_command {
        let remote: McpRemoteSpec = serde_json::from_value(raw.clone())?;
        return Ok(McpServerEntry::Remote(expand_remote_spec(remote)?));
    }

    let s: McpStdioSpec = serde_json::from_value(raw.clone())?;
    if s.command.trim().is_empty() {
        return Err(McpError::config(
            "entrée MCP invalide: fournir `command` (stdio) ou `url` (http/sse)".to_string(),
        ));
    }
    Ok(McpServerEntry::Stdio(expand_stdio_spec(s)?))
}

fn expand_stdio_spec(mut s: McpStdioSpec) -> Result<McpStdioSpec, McpError> {
    s.command = expand_env_in_string(&s.command)?;
    s.args = s
        .args
        .into_iter()
        .map(|a| expand_env_in_string(&a))
        .collect::<Result<_, _>>()?;
    expand_env_in_map(&mut s.env)?;
    Ok(s)
}

fn expand_remote_spec(mut r: McpRemoteSpec) -> Result<McpRemoteSpec, McpError> {
    r.url = expand_env_in_string(&r.url)?;
    if let Some(ref mut h) = r.headers {
        expand_env_in_map(h)?;
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_stdio_minimal() {
        let v = json!({ "command": "npx", "args": ["-y", "pkg"] });
        match parse_server_entry(&v).unwrap() {
            McpServerEntry::Stdio(s) => {
                assert_eq!(s.command, "npx");
                assert_eq!(s.args, vec!["-y", "pkg"]);
            }
            McpServerEntry::Remote(_) => panic!("expected stdio"),
        }
    }

    #[test]
    fn parse_remote_http() {
        let v = json!({
            "type": "http",
            "url": "https://example.invalid/mcp",
            "headers": { "X-Test": "a" }
        });
        match parse_server_entry(&v).unwrap() {
            McpServerEntry::Remote(r) => {
                assert_eq!(r.transport_type_normalized(), "http");
                assert!(r.url.contains("example.invalid"));
            }
            McpServerEntry::Stdio(_) => panic!("expected remote"),
        }
    }

    #[test]
    fn parse_remote_url_only_defaults_http() {
        let v = json!({ "url": "https://example.invalid/mcp" });
        match parse_server_entry(&v).unwrap() {
            McpServerEntry::Remote(r) => {
                assert_eq!(r.transport_type_normalized(), "http");
            }
            McpServerEntry::Stdio(_) => panic!("expected remote"),
        }
    }

    #[test]
    fn load_from_json_round_trip() {
        let j = br#"{"mcpServers":{"x":{"command":"echo"}}}"#;
        let f = McpJsonFile::from_json_slice(j).unwrap();
        assert!(f.mcp_servers.contains_key("x"));
    }
}
