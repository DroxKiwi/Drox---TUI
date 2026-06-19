//! Microcompact — remplace le contenu des anciens `tool_result` par un placeholder court.
//!
//! Aligné sur le leak (`microCompact.ts` / `TIME_BASED_MC_CLEARED_MESSAGE`) :
//! on conserve les **N** derniers résultats d'outils « compactables », le reste
//! est vidé **sans** appel LLM.

use std::collections::HashSet;

use drox_types::{Content, Message, Role};

/// Placeholder injecté à la place d'un ancien `tool_result` (cf. leak).
pub const MICROCOMPACT_CLEARED: &str = "[Old tool result content cleared]";

/// Outils dont les `tool_result` peuvent être vidés (noms canoniques Drox).
const COMPACTABLE_TOOLS: &[&str] = &[
    "file_read",
    "bash",
    "grep",
    "glob",
    "web_search",
    "web_fetch",
    "file_edit",
    "file_write",
    "notebook_edit",
    "lsp",
];

/// Configuration du microcompact.
#[derive(Debug, Clone)]
pub struct MicrocompactConfig {
    /// Nombre de `tool_use` compactables récents à garder intacts.
    pub keep_recent_tools: usize,
    /// Texte de remplacement.
    pub placeholder: String,
}

impl Default for MicrocompactConfig {
    fn default() -> Self {
        Self {
            keep_recent_tools: 3,
            placeholder: MICROCOMPACT_CLEARED.into(),
        }
    }
}

/// Résultat d'une passe microcompact.
#[derive(Debug, Clone, Default)]
pub struct MicrocompactOutcome {
    pub tools_cleared: usize,
    pub blocks_cleared: usize,
}

/// Vide les anciens `tool_result` des outils compactables (ordre chronologique).
pub fn microcompact_messages(
    messages: &mut [Message],
    config: &MicrocompactConfig,
) -> MicrocompactOutcome {
    let compactable_ids = collect_compactable_tool_use_ids(messages);
    if compactable_ids.is_empty() {
        return MicrocompactOutcome::default();
    }

    let keep_recent = config.keep_recent_tools.max(1);
    let keep_from = compactable_ids.len().saturating_sub(keep_recent);
    let clear_ids: HashSet<_> = compactable_ids.into_iter().take(keep_from).collect();
    if clear_ids.is_empty() {
        return MicrocompactOutcome::default();
    }

    let mut blocks_cleared = 0_usize;
    let mut tools_cleared = HashSet::new();

    for msg in messages.iter_mut() {
        if msg.role != Role::Tool {
            continue;
        }
        for block in &mut msg.content {
            let Content::ToolResult {
                tool_use_id,
                content,
                is_error,
            } = block
            else {
                continue;
            };
            if !clear_ids.contains(tool_use_id) {
                continue;
            }
            if *content == config.placeholder || *content == MICROCOMPACT_CLEARED {
                continue;
            }
            tools_cleared.insert(tool_use_id.clone());
            blocks_cleared += 1;
            *content = config.placeholder.clone();
            *is_error = false;
        }
    }

    MicrocompactOutcome {
        tools_cleared: tools_cleared.len(),
        blocks_cleared,
    }
}

/// Collecte les `ToolUseId` compactables dans l'ordre d'apparition (assistant).
fn collect_compactable_tool_use_ids(messages: &[Message]) -> Vec<drox_types::ToolUseId> {
    let mut out = Vec::new();
    for msg in messages {
        if msg.role != Role::Assistant {
            continue;
        }
        for block in &msg.content {
            if let Content::ToolUse { id, name, .. } = block {
                if COMPACTABLE_TOOLS.contains(&name.as_str()) {
                    out.push(id.clone());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_types::{Content, ToolUseId};

    fn assistant_tool_call(name: &str, id: ToolUseId) -> Message {
        Message {
            role: Role::Assistant,
            content: vec![Content::ToolUse {
                id,
                name: name.into(),
                input: serde_json::json!({}),
            }],
        }
    }

    fn tool_result(id: ToolUseId, body: &str) -> Message {
        Message::tool_result(id, body, false)
    }

    #[test]
    fn clears_old_compactable_results_keeps_recent() {
        let id1 = ToolUseId::new();
        let id2 = ToolUseId::new();
        let id3 = ToolUseId::new();
        let id4 = ToolUseId::new();
        let big = "x".repeat(4_000);
        let mut messages = vec![
            assistant_tool_call("file_read", id1.clone()),
            tool_result(id1, &big),
            assistant_tool_call("grep", id2.clone()),
            tool_result(id2, &big),
            assistant_tool_call("file_read", id3.clone()),
            tool_result(id3, &big),
            assistant_tool_call("bash", id4.clone()),
            tool_result(id4, &big),
        ];
        let out = microcompact_messages(
            &mut messages,
            &MicrocompactConfig {
                keep_recent_tools: 2,
                ..MicrocompactConfig::default()
            },
        );
        assert_eq!(out.tools_cleared, 2);
        assert_eq!(out.blocks_cleared, 2);

        let body1 = match &messages[1].content[0] {
            Content::ToolResult { content, .. } => content.clone(),
            _ => panic!(),
        };
        assert_eq!(body1, MICROCOMPACT_CLEARED);

        let body4 = match &messages[7].content[0] {
            Content::ToolResult { content, .. } => content.clone(),
            _ => panic!(),
        };
        assert_eq!(body4, big);
    }

    #[test]
    fn skips_non_compactable_tools() {
        let id = ToolUseId::new();
        let big = "y".repeat(2_000);
        let mut messages = vec![
            assistant_tool_call("todo_write", id.clone()),
            tool_result(id.clone(), &big),
        ];
        let out = microcompact_messages(&mut messages, &MicrocompactConfig::default());
        assert_eq!(out.tools_cleared, 0);
        let body = match &messages[1].content[0] {
            Content::ToolResult { content, .. } => content.clone(),
            _ => panic!(),
        };
        assert_eq!(body, big);
    }
}
