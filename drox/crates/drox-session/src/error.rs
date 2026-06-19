//! Erreurs de persistance session.

use thiserror::Error;

/// Erreurs renvoyées par les API de `drox-session`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SessionError {
    /// Répertoire personnel introuvable (`$HOME` / `%USERPROFILE%`).
    #[error("home directory not found")]
    NoHomeDir,

    /// Chemin non UTF-8 ou invalide pour `camino::Utf8PathBuf`.
    #[error("invalid UTF-8 path")]
    InvalidPath,

    /// Erreur E/S disque.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Erreur de sérialisation JSON.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Identifiant de session invalide pour un nom de fichier.
    #[error("invalid session id: {0}")]
    InvalidSessionId(String),

    /// Fichier de session absent.
    #[error("session file not found: {0}")]
    NotFound(String),
}
