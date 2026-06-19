//! Connexion aux serveurs MCP via `rmcp` (stdio + streamable HTTP).

use std::collections::HashMap;
use std::path::Path;

use http::{HeaderName, HeaderValue};
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess, which_command};
use rmcp::{RoleClient, ServiceExt};

use crate::config::{McpRemoteSpec, McpServerEntry, McpStdioSpec};
use crate::error::McpError;

/// Client MCP actif (`RunningService` rmcp côté **client**).
pub type McpRunningClient = RunningService<RoleClient, ()>;

/// Connecte un serveur **stdio** : spawn le processus et termine le handshake MCP.
pub async fn connect_stdio(spec: &McpStdioSpec) -> Result<McpRunningClient, McpError> {
    let mut cmd = resolve_command(&spec.command)?;
    for a in &spec.args {
        cmd.arg(a);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    let transport = TokioChildProcess::new(cmd)?;
    let client = ().serve(transport).await?;
    Ok(client)
}

/// Connecte un serveur **streamable HTTP** (URL `http`/`https`, schéma MCP moderne).
///
/// Les en-têtes optionnels (ex. `Authorization: Bearer …`) sont passés tels quels
/// au transport reqwest.
#[allow(clippy::implicit_hasher)]
pub async fn connect_streamable_http(
    url: &str,
    headers: &HashMap<String, String>,
) -> Result<McpRunningClient, McpError> {
    let custom = string_headers_to_http(headers)?;
    let cfg = StreamableHttpClientTransportConfig::with_uri(url).custom_headers(custom);
    let transport = StreamableHttpClientTransport::from_config(cfg);
    let client = ().serve(transport).await?;
    Ok(client)
}

/// Dispatch selon le type d'entrée (stdio vs `http`/`sse` distant).
pub async fn connect_server(entry: &McpServerEntry) -> Result<McpRunningClient, McpError> {
    match entry {
        McpServerEntry::Stdio(spec) => connect_stdio(spec).await,
        McpServerEntry::Remote(spec) => connect_remote(spec).await,
    }
}

async fn connect_remote(spec: &McpRemoteSpec) -> Result<McpRunningClient, McpError> {
    let t = spec.transport_type_normalized();
    if t != "http" && t != "sse" {
        return Err(McpError::config(format!(
            "type transport MCP distant non supporté: `{t}` (utiliser `http` ou `sse`)"
        )));
    }
    let hdrs = spec.headers.clone().unwrap_or_default();
    connect_streamable_http(&spec.url, &hdrs).await
}

fn resolve_command(program: &str) -> Result<tokio::process::Command, McpError> {
    let path = Path::new(program);
    if path.is_absolute() || program.contains('/') || program.contains(std::path::MAIN_SEPARATOR) {
        return Ok(tokio::process::Command::new(program));
    }
    which_command(program).map_err(Into::into)
}

fn string_headers_to_http(
    headers: &HashMap<String, String>,
) -> Result<HashMap<HeaderName, HeaderValue>, McpError> {
    let mut out = HashMap::with_capacity(headers.len());
    for (k, v) in headers {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| McpError::invalid_header(format!("nom d'en-tête invalide: {k}")))?;
        let value = HeaderValue::from_str(v)
            .map_err(|_| McpError::invalid_header(format!("valeur d'en-tête invalide pour {k}")))?;
        out.insert(name, value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_headers_round_trip() {
        let mut m = HashMap::new();
        m.insert("X-Foo".to_string(), "bar".to_string());
        let h = string_headers_to_http(&m).unwrap();
        assert_eq!(h.len(), 1);
    }
}
