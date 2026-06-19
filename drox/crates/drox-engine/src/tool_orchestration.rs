//! Partition et exécution par lots des tool calls (cf. leak `toolOrchestration.ts`).
//!
//! - Outils **read-only** / `is_concurrency_safe` consécutifs → lot parallèle.
//! - Mutations et outils non sûrs → lots séquentiels (un appel par lot).

use drox_tools::ToolRegistry;

/// Plafond par défaut de tool calls read-only en parallèle.
pub const DEFAULT_MAX_PARALLEL_TOOL_CALLS: usize = 8;

/// Un lot d'indices dans le tableau de `PendingToolCall` du tour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallBatch {
    /// Exécution parallèle (reads, grep, glob, …).
    Parallel(Vec<usize>),
    /// Exécution strictement série (writes, bash, todo_write, …).
    Serial(Vec<usize>),
}

/// Partitionne les tool calls comme `partitionToolCalls` côté leak.
#[must_use]
pub fn partition_tool_calls(tool_names: &[&str], registry: &ToolRegistry) -> Vec<ToolCallBatch> {
    let mut batches: Vec<ToolCallBatch> = Vec::new();
    for (idx, name) in tool_names.iter().enumerate() {
        let safe = registry.is_concurrency_safe(name);
        if safe {
            if let Some(ToolCallBatch::Parallel(indices)) = batches.last_mut() {
                indices.push(idx);
                continue;
            }
            batches.push(ToolCallBatch::Parallel(vec![idx]));
        } else {
            batches.push(ToolCallBatch::Serial(vec![idx]));
        }
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;
    use drox_tools::ToolRegistry;

    #[test]
    fn groups_consecutive_read_only_tools() {
        let reg = ToolRegistry::with_simple_tools();
        let names = ["file_read", "grep", "glob", "file_edit", "file_read"];
        let refs: Vec<&str> = names.iter().copied().collect();
        let batches = partition_tool_calls(&refs, &reg);
        assert_eq!(
            batches,
            vec![
                ToolCallBatch::Parallel(vec![0, 1, 2]),
                ToolCallBatch::Serial(vec![3]),
                ToolCallBatch::Parallel(vec![4]),
            ]
        );
    }

    #[test]
    fn bash_and_todo_are_serial() {
        let reg = ToolRegistry::with_simple_tools();
        let names = ["bash", "todo_write", "file_read"];
        let refs: Vec<&str> = names.iter().copied().collect();
        let batches = partition_tool_calls(&refs, &reg);
        assert_eq!(
            batches,
            vec![
                ToolCallBatch::Serial(vec![0]),
                ToolCallBatch::Serial(vec![1]),
                ToolCallBatch::Parallel(vec![2]),
            ]
        );
    }
}
