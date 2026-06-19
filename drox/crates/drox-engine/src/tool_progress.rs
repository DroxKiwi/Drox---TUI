//! Pont `ToolProgressSink` → `AgentEvent::ToolProgress`.

use std::sync::Arc;

use drox_tools::{ShellProgressUpdate, ToolProgressSink};
use drox_types::ToolUseId;
use tokio::sync::mpsc;

use crate::event::AgentEvent;
use crate::EngineError;

/// Forwarder branché sur le `ToolContext` pendant un tool call.
#[derive(Clone)]
pub struct ToolProgressBridge {
    tx: mpsc::Sender<Result<AgentEvent, EngineError>>,
    id: ToolUseId,
    name: String,
}

impl ToolProgressBridge {
    #[must_use]
    pub fn new(
        tx: mpsc::Sender<Result<AgentEvent, EngineError>>,
        id: ToolUseId,
        name: impl Into<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            tx,
            id,
            name: name.into(),
        })
    }
}

impl ToolProgressSink for ToolProgressBridge {
    fn report_shell(&self, update: ShellProgressUpdate) {
        let _ = self.tx.try_send(Ok(AgentEvent::ToolProgress {
            id: self.id.clone(),
            name: self.name.clone(),
            output: update.output,
            full_output: update.full_output,
            elapsed_ms: update.elapsed_ms,
            total_lines: update.total_lines,
        }));
    }
}
