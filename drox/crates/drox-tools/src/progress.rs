//! Rapports de progression outil (ex. sortie bash streamée).

/// Mise à jour partielle d'une commande shell.
#[derive(Debug, Clone)]
pub struct ShellProgressUpdate {
    /// Dernières lignes affichables (souvent 5).
    pub output: String,
    /// Sortie cumulée complète.
    pub full_output: String,
    pub elapsed_ms: u64,
    pub total_lines: usize,
}

/// Sink optionnel branché par le moteur pendant l'exécution d'un tool.
pub trait ToolProgressSink: Send + Sync {
    fn report_shell(&self, update: ShellProgressUpdate);
}
