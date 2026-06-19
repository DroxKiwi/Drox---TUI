//! Noms qualifiés des tools MCP (`mcp__<server>__<tool>`), alignés sur le leak.

const MAX_SEGMENT_LEN: usize = 64;

/// Normalise un segment pour l'API / les noms de tools (`^[a-zA-Z0-9_-]{1,64}$`).
#[must_use]
pub fn normalize_name_for_mcp(name: &str) -> String {
    let mut normalized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if normalized.len() > MAX_SEGMENT_LEN {
        normalized.truncate(MAX_SEGMENT_LEN);
    }
    if normalized.is_empty() {
        normalized.push('_');
    }
    normalized
}

/// Préfixe `mcp__<server>__` pour un serveur donné.
#[must_use]
pub fn mcp_tool_prefix(server_name: &str) -> String {
    format!("mcp__{}__", normalize_name_for_mcp(server_name))
}

/// Nom complet exposé au modèle : `mcp__<server>__<tool>`.
#[must_use]
pub fn build_mcp_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "{}{}",
        mcp_tool_prefix(server_name),
        normalize_name_for_mcp(tool_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_replaces_invalid_chars() {
        assert_eq!(normalize_name_for_mcp("my.server"), "my_server");
        assert_eq!(normalize_name_for_mcp("a b"), "a_b");
    }

    #[test]
    fn build_qualified_name() {
        assert_eq!(
            build_mcp_tool_name("filesystem", "read_file"),
            "mcp__filesystem__read_file"
        );
    }
}
