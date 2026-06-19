//! Mode bash intégré (`!`) — exécution shell directe sans tour agent.

use drox_tools::{ShellProgressUpdate, ToolProgressSink};
use tokio::sync::mpsc;

/// Sink de progression pour le mode `!` (mise à jour `ActiveBashRun` côté UI).
#[derive(Clone)]
pub struct BashModeProgressSink {
    tx: mpsc::UnboundedSender<(String, usize)>,
}

impl BashModeProgressSink {
    #[must_use]
    pub fn channel() -> (Self, mpsc::UnboundedReceiver<(String, usize)>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl ToolProgressSink for BashModeProgressSink {
    fn report_shell(&self, update: ShellProgressUpdate) {
        let _ = self.tx.send((update.output, update.total_lines));
    }
}
