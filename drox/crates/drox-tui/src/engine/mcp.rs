//! Introspection et test MCP (`/mcp`).

use camino::{Utf8Path, Utf8PathBuf};
use drox_mcp::{McpHub, McpJsonFile, McpServerEntry, parse_server_entry};
use serde_json::Value;

use super::EngineRuntime;

#[derive(Debug, Clone)]
pub struct McpServerLine {
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct McpPanelSnapshot {
    pub config_path: String,
    pub servers: Vec<McpServerLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum McpAction {
    List,
    Tools { server: Option<String> },
    Resources { server: Option<String> },
    Ping { server: String },
    Add { kind: String },
    Browse { server: Option<String> },
}

impl EngineRuntime {
    /// Exécute `/mcp` et sous-commandes.
    pub async fn run_mcp_command(&self, args: &str) -> Vec<String> {
        match parse_mcp_args(args) {
            McpAction::List => self.format_mcp_list().await,
            McpAction::Tools { server } => self.format_mcp_tools(server.as_deref()).await,
            McpAction::Resources { server } => self.format_mcp_resources(server.as_deref()).await,
            McpAction::Ping { server } if server.is_empty() => vec![
                "Usage : /mcp ping <serveur>".into(),
                "Astuce : /mcp · /mcp tools [serveur] · /mcp resources [serveur]".into(),
            ],
            McpAction::Ping { server } => self.ping_mcp_server(&server).await,
            McpAction::Add { kind } => format_mcp_add_guide(&kind),
            McpAction::Browse { server } => self.format_mcp_tools(server.as_deref()).await,
        }
    }

    async fn format_mcp_list(&self) -> Vec<String> {
        let mut lines = vec!["Serveurs MCP (config workspace)".into()];
        match load_mcp_config(&self.workspace).await {
            Ok((path, file)) => {
                lines.push(format!("  config : {path}"));
                if file.mcp_servers.is_empty() {
                    lines.push("  (mcpServers vide)".into());
                    return lines;
                }
                let mut names: Vec<_> = file.mcp_servers.keys().cloned().collect();
                names.sort();
                for name in names {
                    let raw = file.mcp_servers.get(&name).expect("key exists");
                    lines.push(format!("  — {name} : {}", summarize_server_entry(raw)));
                }
                lines.push(
                    "Astuce : /mcp tools [serveur] · /mcp browse · /mcp add stdio|remote · /mcp ping <serveur>"
                        .into(),
                );
            }
            Err(msg) => {
                lines.push(format!("  {msg}"));
                lines.push("  Créer `.mcp.json` ou `mcp.json` à la racine du workspace.".into());
            }
        }
        lines
    }

    async fn format_mcp_tools(&self, server: Option<&str>) -> Vec<String> {
        let Some(hub) = McpHub::discover(&self.workspace).await else {
            return vec!["Aucun MCP configuré (.mcp.json / mcp.json).".into()];
        };

        let servers: Vec<String> = match server {
            Some(s) => {
                if !hub.server_names().iter().any(|n| n == s) {
                    return vec![format!("Serveur MCP inconnu : `{s}`")];
                }
                vec![s.to_string()]
            }
            None => hub.server_names(),
        };

        let mut lines = vec!["Tools MCP (connexion live)".into()];
        let mut total = 0usize;
        for name in servers {
            match hub.list_tools(&name).await {
                Ok(tools) => {
                    lines.push(format!("— {name} ({} tool(s))", tools.len()));
                    for tool in &tools {
                        lines.push(format_tool_line(&name, tool));
                        total += 1;
                    }
                }
                Err(e) => lines.push(format!("— {name} : échec list_tools — {e}")),
            }
        }
        lines.push(format!("Total : {total} tool(s)"));
        lines
    }

    async fn format_mcp_resources(&self, server: Option<&str>) -> Vec<String> {
        let Some(hub) = McpHub::discover(&self.workspace).await else {
            return vec!["Aucun MCP configuré (.mcp.json / mcp.json).".into()];
        };

        match hub.list_resources(server).await {
            Ok(resources) => {
                let mut lines = vec![format!(
                    "Ressources MCP{}",
                    server
                        .map(|s| format!(" — {s}"))
                        .unwrap_or_default()
                )];
                if resources.is_empty() {
                    lines.push("  (aucune ressource)".into());
                    return lines;
                }
                for res in resources.iter().take(40) {
                    lines.push(format_resource_line(res));
                }
                if resources.len() > 40 {
                    lines.push(format!("  … +{} ressources", resources.len() - 40));
                }
                lines.push(format!("Total : {} ressource(s)", resources.len()));
                lines
            }
            Err(e) => vec![format!("list_resources : {e}")],
        }
    }

    async fn ping_mcp_server(&self, server: &str) -> Vec<String> {
        let Some(hub) = McpHub::discover(&self.workspace).await else {
            return vec!["Aucun MCP configuré.".into()];
        };
        if !hub.server_names().iter().any(|n| n == server) {
            return vec![format!("Serveur inconnu : `{server}`")];
        }
        match hub.list_tools(server).await {
            Ok(tools) => vec![format!(
                "[OK] `{server}` — connexion OK, {} tool(s) découverts",
                tools.len()
            )],
            Err(e) => vec![format!("[FAIL] `{server}` — {e} — vérifiez la config ou relancez avec /mcp ping")],
        }
    }

    /// Snapshot pour le panneau MCP (config workspace).
    pub async fn build_mcp_panel_snapshot(&self) -> Option<McpPanelSnapshot> {
        let (path, file) = load_mcp_config(&self.workspace).await.ok()?;
        if file.mcp_servers.is_empty() {
            return None;
        }
        let mut servers = Vec::new();
        let mut names: Vec<_> = file.mcp_servers.keys().cloned().collect();
        names.sort();
        for name in names {
            let raw = file.mcp_servers.get(&name).expect("key");
            servers.push(McpServerLine {
                name,
                summary: summarize_server_entry(raw),
            });
        }
        Some(McpPanelSnapshot {
            config_path: path.to_string(),
            servers,
        })
    }
}

fn parse_mcp_args(args: &str) -> McpAction {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return McpAction::List;
    }
    match parts[0].to_ascii_lowercase().as_str() {
        "tools" => McpAction::Tools {
            server: parts.get(1).map(|s| (*s).to_string()),
        },
        "resources" | "res" => McpAction::Resources {
            server: parts.get(1).map(|s| (*s).to_string()),
        },
        "ping" | "connect" | "reconnect" => McpAction::Ping {
            server: parts.get(1).unwrap_or(&"").to_string(),
        },
        "add" => McpAction::Add {
            kind: parts.get(1).unwrap_or(&"").to_string(),
        },
        "browse" => McpAction::Browse {
            server: parts.get(1).map(|s| (*s).to_string()),
        },
        _ if parts.len() == 1 => McpAction::Tools {
            server: Some(parts[0].to_string()),
        },
        _ => McpAction::List,
    }
}

async fn load_mcp_config(workspace: &Utf8Path) -> Result<(Utf8PathBuf, McpJsonFile), String> {
    for path in [workspace.join(".mcp.json"), workspace.join("mcp.json")] {
        if !path.is_file() {
            continue;
        }
        let file = McpJsonFile::load(path.as_std_path())
            .await
            .map_err(|e| format!("{path} : {e}"))?;
        return Ok((path, file));
    }
    Err("aucun .mcp.json ni mcp.json".into())
}

#[must_use]
fn format_mcp_add_guide(kind: &str) -> Vec<String> {
    let kind = kind.to_ascii_lowercase();
    match kind.as_str() {
        "stdio" => vec![
            "Modèle serveur MCP stdio — ajoutez dans `.mcp.json` :".into(),
            r#"  "mon-serveur": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/chemin"]
  }"#
                .into(),
            "Puis : /mcp ping mon-serveur · /mcp tools".into(),
        ],
        "remote" | "http" | "sse" => vec![
            "Modèle serveur MCP distant — ajoutez dans `.mcp.json` :".into(),
            r#"  "mon-api": {
    "type": "http",
    "url": "https://example.com/mcp",
    "headers": { "Authorization": "Bearer ${MON_TOKEN}" }
  }"#
                .into(),
            "Variables `${VAR}` développées depuis l'environnement.".into(),
        ],
        _ => vec![
            "Usage : /mcp add stdio | /mcp add remote".into(),
            "Éditez `.mcp.json` à la racine du workspace (clé `mcpServers`).".into(),
            "Voir aussi : /mcp · /mcp tools · /mcp browse · /mcp ping <nom>".into(),
        ],
    }
}

fn summarize_server_entry(raw: &Value) -> String {
    match parse_server_entry(raw) {
        Ok(McpServerEntry::Stdio(s)) => {
            let args = if s.args.is_empty() {
                String::new()
            } else {
                format!(" {}", s.args.join(" "))
            };
            format!("stdio `{}`{args}", s.command)
        }
        Ok(McpServerEntry::Remote(r)) => {
            format!("{} {}", r.transport_type_normalized(), r.url)
        }
        Err(e) => format!("invalide — {e}"),
    }
}

fn format_tool_line(server: &str, tool: &Value) -> String {
    let remote = tool
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("?");
    let qualified = drox_mcp::build_mcp_tool_name(server, remote);
    let desc = tool
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| truncate(d, 60))
        .unwrap_or_default();
    if desc.is_empty() {
        format!("    {qualified}")
    } else {
        format!("    {qualified} — {desc}")
    }
}

fn format_resource_line(res: &Value) -> String {
    let server = res
        .get("server")
        .and_then(|s| s.as_str())
        .unwrap_or("?");
    let uri = res
        .get("uri")
        .and_then(|u| u.as_str())
        .unwrap_or("?");
    let name = res
        .get("name")
        .and_then(|n| n.as_str())
        .map(|n| format!(" ({n})"))
        .unwrap_or_default();
    format!("  [{server}] {uri}{name}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_is_list() {
        assert_eq!(parse_mcp_args(""), McpAction::List);
    }

    #[test]
    fn parse_tools_with_server() {
        assert_eq!(
            parse_mcp_args("tools fs"),
            McpAction::Tools {
                server: Some("fs".into())
            }
        );
    }

    #[test]
    fn parse_shorthand_server_tools() {
        assert_eq!(
            parse_mcp_args("myserver"),
            McpAction::Tools {
                server: Some("myserver".into())
            }
        );
    }

    #[test]
    fn parse_ping() {
        assert_eq!(
            parse_mcp_args("ping sentry"),
            McpAction::Ping {
                server: "sentry".into()
            }
        );
    }
}
