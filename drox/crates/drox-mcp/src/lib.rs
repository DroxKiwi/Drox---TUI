//! `drox-mcp` — couche d'orchestration MCP.
//!
//! - Chargement de fichiers style `.mcp.json` (`mcpServers`) avec expansion `${VAR}`.
//! - Connexion **stdio** (processus enfant) et **HTTP/SSE** (transport streamable
//!   HTTP du SDK `rmcp`).
//! - Helpers RPC `list_tools_json` / `call_tool_json` pour intégration avec le
//!   moteur agent (sprints ultérieurs).
//! - Réexport des types OAuth de `rmcp` (`AuthClient`, `AuthorizationManager`, …)
//!   pour les clients qui implémentent un flux complet.
//!
//! Voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 2.4.

mod config;
mod connect;
mod env_expand;
mod error;
mod hub;
mod names;
pub mod oauth;
mod rpc;

pub use config::{McpJsonFile, McpRemoteSpec, McpServerEntry, McpStdioSpec, parse_server_entry};
pub use connect::{McpRunningClient, connect_server, connect_stdio, connect_streamable_http};
pub use env_expand::{expand_env_in_map, expand_env_in_string};
pub use error::McpError;
pub use hub::{McpHub, McpToolStub};
pub use names::{build_mcp_tool_name, mcp_tool_prefix, normalize_name_for_mcp};
pub use rpc::{
    call_tool_json, call_tool_object, list_resources_json, list_resources_json_or_empty,
    list_tools_json,
};
