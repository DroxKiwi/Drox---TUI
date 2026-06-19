//! Bloc appel d'outil (équivalent `tool_start` / `tool_finish` UI).

use serde_json::Value;

/// Entrée d'outil dans le fil TUI.
#[derive(Debug, Clone)]
pub struct ToolCallBlock {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub output: Option<Value>,
    pub is_error: bool,
    pub collapsed: bool,
}

impl ToolCallBlock {
    #[must_use]
    pub fn started(id: String, name: String, arguments: Value) -> Self {
        Self {
            id,
            name,
            arguments,
            output: None,
            is_error: false,
            collapsed: true,
        }
    }

    pub fn finish(&mut self, output: Value, is_error: bool) {
        self.output = Some(output);
        self.is_error = is_error;
    }
}
