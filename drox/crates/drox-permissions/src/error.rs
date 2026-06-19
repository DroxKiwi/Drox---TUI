//! Erreurs propres au moteur de permissions.

use thiserror::Error;

/// Erreur retournée par les API publiques de `drox-permissions`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PermissionError {
    /// Échec de lecture / parsing d'un fichier de configuration.
    #[error("invalid permission config: {0}")]
    InvalidConfig(String),

    /// Erreur d'I/O sur un fichier de settings.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Erreur de désérialisation JSON.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}
