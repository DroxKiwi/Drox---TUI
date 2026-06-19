//! Formatage fil pour tools MCP (`mcp__*`, `mcp_call`, ressources).

use drox_types::ToolUseId;
use serde_json::Value;

use super::lines_viewer::LinesViewerState;
use super::tool_output::{format_error, push_wrapped_body, push_wrapped_body_expandable};

const MAX_MCP_RESOURCES: usize = 20;
const MAX_BODY_LINES: usize = 20;
const LINE_MAX: usize = 120;

#[must_use]
pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp__")
        || matches!(
            name,
            "mcp_call" | "list_mcp_resources" | "read_mcp_resource"
        )
}

#[must_use]
pub fn format_mcp_start(name: &str, id: &ToolUseId, args: &Value) -> Vec<String> {
    if let Some((server, tool)) = parse_mcp_qualified(name) {
        let short = short_args(args);
        return vec![format!("▸ mcp ({id}) {server}::{tool}{short}")];
    }
    match name {
        "mcp_call" => {
            let server = args.get("server").and_then(Value::as_str).unwrap_or("?");
            let tool = args.get("tool").and_then(Value::as_str).unwrap_or("?");
            vec![format!("▸ mcp_call ({id}) {server}::{tool}")]
        }
        "list_mcp_resources" => {
            let server = args.get("server").and_then(Value::as_str);
            match server {
                Some(s) => vec![format!("▸ list_mcp_resources ({id}) [{s}]")],
                None => vec![format!("▸ list_mcp_resources ({id}) [tous]")],
            }
        }
        "read_mcp_resource" => {
            let server = args.get("server").and_then(Value::as_str).unwrap_or("?");
            let uri = args.get("uri").and_then(Value::as_str).unwrap_or("?");
            vec![format!("▸ read_mcp_resource ({id}) {server} {uri}")]
        }
        _ => vec![format!("▸ {name} ({id})")],
    }
}

#[must_use]
pub fn format_mcp_finish(name: &str, id: &ToolUseId, output: &Value, is_error: bool) -> Vec<String> {
    if is_error {
        let label = display_name(name);
        return format_error(&label, id, output);
    }
    if name == "list_mcp_resources" {
        return format_list_resources_finish(id, output);
    }
    if name == "read_mcp_resource" {
        return format_read_resource_finish(id, output);
    }
    format_call_result_finish(name, id, output)
}

fn display_name(name: &str) -> String {
    if let Some((server, tool)) = parse_mcp_qualified(name) {
        format!("mcp {server}::{tool}")
    } else {
        name.to_string()
    }
}

fn format_list_resources_finish(id: &ToolUseId, output: &Value) -> Vec<String> {
    let resources = output
        .get("resources")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut lines = vec![format!(
        "◂ list_mcp_resources ({id}) {} ressource(s)",
        resources.len()
    )];
    for r in resources.iter().take(MAX_MCP_RESOURCES) {
        let server = r.get("server").and_then(Value::as_str).unwrap_or("?");
        let uri = r.get("uri").and_then(Value::as_str).unwrap_or("?");
        let title = r.get("name").and_then(Value::as_str).unwrap_or("");
        if title.is_empty() {
            lines.push(format!("    [{server}] {uri}"));
        } else {
            lines.push(format!(
                "    [{server}] {} — {}",
                truncate(title, LINE_MAX),
                truncate(uri, LINE_MAX)
            ));
        }
    }
    if resources.len() > MAX_MCP_RESOURCES {
        lines.push(format!(
            "    … +{} ressources (e pour parcourir)",
            resources.len() - MAX_MCP_RESOURCES
        ));
    }
    lines
}

#[must_use]
pub fn mcp_is_expandable(name: &str, output: &Value) -> bool {
    if output.get("_truncated").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    if name == "list_mcp_resources" {
        return output
            .get("resources")
            .and_then(Value::as_array)
            .is_some_and(|r| r.len() > MAX_MCP_RESOURCES);
    }
    let body = collect_mcp_body(name, output);
    body.len() > MAX_BODY_LINES
}

#[must_use]
pub fn mcp_viewer_from_output(
    name: &str,
    id: ToolUseId,
    output: &Value,
) -> Option<LinesViewerState> {
    let body = collect_mcp_body(name, output);
    if body.is_empty() {
        return None;
    }
    let title = if let Some((server, tool)) = parse_mcp_qualified(name) {
        format!(" mcp — {server}::{tool} ")
    } else {
        format!(" {name} ")
    };
    Some(LinesViewerState::from_plain(id, title, body))
}

fn format_read_resource_finish(id: &ToolUseId, output: &Value) -> Vec<String> {
    let mut lines = vec![format!("◂ read_mcp_resource ({id}) ok")];
    if let Some(contents) = output.get("contents").and_then(Value::as_array) {
        append_mcp_content_blocks(&mut lines, contents);
    } else {
        push_json_preview(&mut lines, output);
    }
    lines
}

fn format_call_result_finish(name: &str, id: &ToolUseId, output: &Value) -> Vec<String> {
    if output.get("_truncated").and_then(Value::as_bool) == Some(true) {
        let orig = output
            .get("original_chars")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let max = output.get("max_chars").and_then(Value::as_u64).unwrap_or(0);
        let mut lines = vec![finish_header(name, id, &format!("[tronqué {orig}→{max} chars]"))];
        if let Some(preview) = output.get("preview").and_then(Value::as_str) {
            push_wrapped_body_expandable(&mut lines, preview, MAX_BODY_LINES);
        }
        return lines;
    }

    let mcp_err = output.get("isError").and_then(Value::as_bool) == Some(true)
        || output.get("is_error").and_then(Value::as_bool) == Some(true);
    let mut lines = vec![finish_header(
        name,
        id,
        if mcp_err { "[erreur MCP]" } else { "ok" },
    )];

    if let Some(contents) = output.get("content").and_then(Value::as_array) {
        append_mcp_content_blocks(&mut lines, contents);
    } else {
        push_json_preview(&mut lines, output);
    }
    lines
}

fn finish_header(name: &str, id: &ToolUseId, suffix: &str) -> String {
    if let Some((server, tool)) = parse_mcp_qualified(name) {
        format!("◂ mcp ({id}) {server}::{tool} {suffix}")
    } else {
        format!("◂ {name} ({id}) {suffix}")
    }
}

fn append_mcp_content_blocks(lines: &mut Vec<String>, blocks: &[Value]) {
    append_mcp_content_blocks_inner(lines, blocks, Some(MAX_BODY_LINES), true);
}

fn collect_mcp_body(name: &str, output: &Value) -> Vec<String> {
    if output.get("_truncated").and_then(Value::as_bool) == Some(true) {
        return output
            .get("preview")
            .and_then(Value::as_str)
            .map(|p| p.lines().map(str::to_string).collect())
            .unwrap_or_default();
    }
    if name == "list_mcp_resources" {
        return output
            .get("resources")
            .and_then(Value::as_array)
            .map(|resources| {
                resources
                    .iter()
                    .map(|r| {
                        let server = r.get("server").and_then(Value::as_str).unwrap_or("?");
                        let uri = r.get("uri").and_then(Value::as_str).unwrap_or("?");
                        let title = r.get("name").and_then(Value::as_str).unwrap_or("");
                        if title.is_empty() {
                            format!("[{server}] {uri}")
                        } else {
                            format!(
                                "[{server}] {} — {}",
                                truncate(title, LINE_MAX),
                                truncate(uri, LINE_MAX)
                            )
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    if name == "read_mcp_resource" {
        let mut lines = Vec::new();
        if let Some(contents) = output.get("contents").and_then(Value::as_array) {
            collect_mcp_content_blocks(&mut lines, contents);
        } else {
            let body = serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".into());
            lines.extend(body.lines().map(str::to_string));
        }
        return lines;
    }
    let mut lines = Vec::new();
    if let Some(contents) = output.get("content").and_then(Value::as_array) {
        collect_mcp_content_blocks(&mut lines, contents);
    } else {
        let body = serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".into());
        lines.extend(body.lines().map(str::to_string));
    }
    lines
}

fn collect_mcp_content_blocks(lines: &mut Vec<String>, blocks: &[Value]) {
    for (i, block) in blocks.iter().enumerate() {
        let ty = block.get("type").and_then(Value::as_str).unwrap_or("?");
        match ty {
            "text" => {
                if blocks.len() > 1 {
                    lines.push(format!("[{i}] text:"));
                }
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    lines.extend(text.lines().map(str::to_string));
                }
            }
            "image" => {
                let mime = block
                    .get("mimeType")
                    .or_else(|| block.get("mime_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                let data_len = block
                    .get("data")
                    .and_then(Value::as_str)
                    .map(str::len)
                    .unwrap_or(0);
                lines.push(format!("[{i}] image ({mime}, {data_len} b64)"));
            }
            "resource" => {
                let uri = block
                    .get("resource")
                    .and_then(|r| r.get("uri"))
                    .and_then(Value::as_str)
                    .or_else(|| block.get("uri").and_then(Value::as_str))
                    .unwrap_or("?");
                lines.push(format!("[{i}] resource {uri}"));
            }
            other => lines.push(format!("[{i}] {other}")),
        }
    }
}

fn append_mcp_content_blocks_inner(
    lines: &mut Vec<String>,
    blocks: &[Value],
    max_text_lines: Option<usize>,
    expandable: bool,
) {
    for (i, block) in blocks.iter().enumerate() {
        let ty = block.get("type").and_then(Value::as_str).unwrap_or("?");
        match ty {
            "text" => {
                if blocks.len() > 1 {
                    lines.push(format!("    [{i}] text:"));
                }
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    match max_text_lines {
                        Some(max) if expandable => {
                            push_wrapped_body_expandable(lines, text, max);
                        }
                        Some(max) => push_wrapped_body(lines, text, max),
                        None => lines.extend(text.lines().map(|l| format!("    {l}"))),
                    }
                }
            }
            "image" => {
                let mime = block
                    .get("mimeType")
                    .or_else(|| block.get("mime_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                let data_len = block
                    .get("data")
                    .and_then(Value::as_str)
                    .map(str::len)
                    .unwrap_or(0);
                lines.push(format!("    [{i}] image ({mime}, {data_len} b64)"));
            }
            "resource" => {
                let uri = block
                    .get("resource")
                    .and_then(|r| r.get("uri"))
                    .and_then(Value::as_str)
                    .or_else(|| block.get("uri").and_then(Value::as_str))
                    .unwrap_or("?");
                lines.push(format!("    [{i}] resource {uri}"));
            }
            other => lines.push(format!("    [{i}] {other}")),
        }
    }
}

fn push_json_preview(lines: &mut Vec<String>, output: &Value) {
    let body = serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".into());
    push_wrapped_body(lines, &body, MAX_BODY_LINES);
}

fn parse_mcp_qualified(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    rest.split_once("__")
}

fn short_args(args: &Value) -> String {
    let body = serde_json::to_string(args).unwrap_or_else(|_| "{}".into());
    if body == "{}" || body == "null" {
        return String::new();
    }
    if body.len() > 80 {
        format!(" {}…", &body[..80])
    } else {
        format!(" {body}")
    }
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
    use drox_types::ToolUseId;

    #[test]
    fn detects_mcp_tools() {
        assert!(is_mcp_tool("mcp__fs__read"));
        assert!(is_mcp_tool("mcp_call"));
        assert!(!is_mcp_tool("file_read"));
    }

    #[test]
    fn formats_text_content_blocks() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "content": [{ "type": "text", "text": "hello MCP" }],
            "isError": false
        });
        let lines = format_mcp_finish("mcp__srv__tool", &id, &out, false);
        assert!(lines.iter().any(|l| l.contains("hello MCP")));
        assert!(lines.iter().any(|l| l.contains("srv::tool")));
    }

    #[test]
    fn formats_resource_list() {
        let id = ToolUseId::new();
        let out = serde_json::json!({
            "resources": [{
                "server": "docs",
                "uri": "file:///readme.md",
                "name": "Readme"
            }]
        });
        let lines = format_mcp_finish("list_mcp_resources", &id, &out, false);
        assert!(lines.iter().any(|l| l.contains("readme.md")));
    }
}
