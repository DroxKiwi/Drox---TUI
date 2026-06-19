//! Tools MCP — stubs dynamiques `mcp__*`, `list_mcp_resources`, `read_mcp_resource`, fallback `mcp_call`.

use std::sync::Arc;

use async_trait::async_trait;
use drox_mcp::{McpHub, McpToolStub};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::ToolRegistry;
use crate::tool::Tool;

const MAX_RESULT_CHARS: usize = 100_000;

fn require_hub(ctx: &ToolContext) -> Result<&McpHub, ToolError> {
    let Some(hub) = ctx.mcp_hub.as_deref() else {
        return Err(ToolError::mcp(
            "aucun serveur MCP configuré (ajouter `.mcp.json` ou `mcp.json` à la racine du workspace)",
        ));
    };
    Ok(hub)
}

fn truncate_value(value: Value) -> Value {
    let serialized = serde_json::to_string(&value).unwrap_or_default();
    if serialized.len() <= MAX_RESULT_CHARS {
        return value;
    }
    let end = serialized
        .char_indices()
        .nth(MAX_RESULT_CHARS)
        .map_or(serialized.len(), |(i, _)| i);
    json!({
        "_truncated": true,
        "max_chars": MAX_RESULT_CHARS,
        "original_chars": serialized.len(),
        "preview": &serialized[..end],
    })
}

/// Stub dynamique : un tool MCP distant enregistré sous `mcp__<server>__<tool>`.
pub struct McpDynamicTool {
    qualified_name: String,
    server: String,
    remote_tool: String,
    description: String,
    input_schema: Value,
    read_only: bool,
}

impl McpDynamicTool {
    #[must_use]
    pub fn from_stub(stub: McpToolStub) -> Self {
        Self {
            qualified_name: stub.qualified_name,
            server: stub.server,
            remote_tool: stub.remote_name,
            description: stub.description,
            input_schema: stub.input_schema,
            read_only: stub.read_only,
        }
    }
}

#[async_trait]
impl Tool for McpDynamicTool {
    fn name(&self) -> &str {
        &self.qualified_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let hub = require_hub(ctx)?;
        let arguments = match input {
            Value::Null => None,
            Value::Object(m) if m.is_empty() => None,
            other => Some(other),
        };
        let result = hub
            .call_tool(&self.server, &self.remote_tool, arguments)
            .await
            .map_err(|e| ToolError::mcp(e.to_string()))?;
        Ok(truncate_value(result))
    }
}

/// Enregistre les stubs `mcp__*` + tools ressources ; `mcp_call` seulement si aucun stub.
pub async fn register_mcp_tools(registry: &mut ToolRegistry, hub: &McpHub) -> usize {
    registry.register(Arc::new(ListMcpResourcesTool));
    registry.register(Arc::new(ReadMcpResourceTool));

    let stubs = hub.discover_tool_stubs().await;
    let mut seen = std::collections::HashSet::new();
    for stub in stubs {
        if !seen.insert(stub.qualified_name.clone()) {
            continue;
        }
        registry.register(Arc::new(McpDynamicTool::from_stub(stub)));
    }
    let count = seen.len();
    if count == 0 {
        registry.register(Arc::new(McpCallTool));
    }
    count
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpCallInput {
    pub server: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

pub struct McpCallTool;

#[async_trait]
impl Tool for McpCallTool {
    fn name(&self) -> &str {
        "mcp_call"
    }

    fn description(&self) -> &str {
        "Invoque un tool sur un serveur MCP configuré (`.mcp.json`). \
         Utilisé en secours si les stubs `mcp__*` n'ont pas pu être chargés."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(McpCallInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: McpCallInput = serde_json::from_value(input)?;
        if args.server.trim().is_empty() || args.tool.trim().is_empty() {
            return Err(ToolError::invalid_args(
                "`server` et `tool` sont obligatoires",
            ));
        }
        let hub = require_hub(ctx)?;
        let result = hub
            .call_tool(&args.server, &args.tool, args.arguments)
            .await
            .map_err(|e| ToolError::mcp(e.to_string()))?;
        Ok(truncate_value(result))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListMcpResourcesInput {
    pub server: Option<String>,
}

pub struct ListMcpResourcesTool;

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "list_mcp_resources"
    }

    fn description(&self) -> &str {
        "Liste les ressources MCP disponibles. Chaque entrée inclut un champ `server`."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(ListMcpResourcesInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: ListMcpResourcesInput = serde_json::from_value(input)?;
        let hub = require_hub(ctx)?;
        let resources = hub
            .list_resources(args.server.as_deref())
            .await
            .map_err(|e| ToolError::mcp(e.to_string()))?;
        Ok(truncate_value(json!({ "resources": resources })))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMcpResourceInput {
    pub server: String,
    pub uri: String,
}

pub struct ReadMcpResourceTool;

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "read_mcp_resource"
    }

    fn description(&self) -> &str {
        "Lit une ressource MCP par URI sur le serveur indiqué."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(ReadMcpResourceInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: ReadMcpResourceInput = serde_json::from_value(input)?;
        if args.server.trim().is_empty() || args.uri.trim().is_empty() {
            return Err(ToolError::invalid_args(
                "`server` et `uri` sont obligatoires",
            ));
        }
        let hub = require_hub(ctx)?;
        let result = hub
            .read_resource(&args.server, &args.uri)
            .await
            .map_err(|e| ToolError::mcp(e.to_string()))?;
        Ok(truncate_value(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_mcp::build_mcp_tool_name;

    #[test]
    fn dynamic_tool_uses_qualified_name() {
        let tool = McpDynamicTool {
            qualified_name: build_mcp_tool_name("fs", "read_file"),
            server: "fs".into(),
            remote_tool: "read_file".into(),
            description: "Read".into(),
            input_schema: json!({"type": "object"}),
            read_only: true,
        };
        assert_eq!(tool.name(), "mcp__fs__read_file");
        assert!(tool.is_read_only());
    }
}
