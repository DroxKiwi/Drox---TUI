//! Erreurs des tools filesystem / recherche / réseau.

use std::io;

use camino::Utf8PathBuf;
use thiserror::Error;

/// Erreur renvoyée par l'exécution d'un tool.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ToolError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    #[error("invalid JSON input: {0}")]
    InvalidInput(#[from] serde_json::Error),

    #[error("invalid tool arguments: {0}")]
    InvalidArgs(String),

    #[error("path escapes workspace: {path}")]
    PathEscape { path: Utf8PathBuf },

    #[error("I/O error on {path}: {source}")]
    Io {
        path: Utf8PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("regex compilation error: {0}")]
    Regex(#[from] regex::Error),

    #[error("glob pattern error: {0}")]
    Glob(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("interactive prompt unavailable: {0}")]
    Interactive(String),

    #[error("plan mode forbids write operation: {0}")]
    PlanModeViolation(String),

    #[error("edit failed: {0}")]
    EditFailed(String),

    /// Le tool est exécuté par un client distant (extension VS Code…) et ce
    /// dernier a rapporté une erreur ou s'est déconnecté.
    #[error("remote tool error: {0}")]
    Remote(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    /// Chemin bloqué par `.droxignore` (§2.34).
    #[error("path blocked by .droxignore: {path}")]
    DroxIgnore { path: Utf8PathBuf },
}

impl ToolError {
    pub fn invalid_args(msg: impl Into<String>) -> Self {
        Self::InvalidArgs(msg.into())
    }

    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    pub fn interactive(msg: impl Into<String>) -> Self {
        Self::Interactive(msg.into())
    }

    pub fn plan_violation(tool: impl Into<String>) -> Self {
        Self::PlanModeViolation(tool.into())
    }

    pub fn edit_failed(msg: impl Into<String>) -> Self {
        Self::EditFailed(msg.into())
    }

    pub fn remote(msg: impl Into<String>) -> Self {
        Self::Remote(msg.into())
    }

    pub fn mcp(msg: impl Into<String>) -> Self {
        Self::Mcp(msg.into())
    }

    pub fn drox_ignore(path: Utf8PathBuf) -> Self {
        Self::DroxIgnore { path }
    }

    #[allow(clippy::missing_const_for_fn)] // `io::Error` n'est pas compatible `const`
    pub fn io(path: Utf8PathBuf, source: io::Error) -> Self {
        Self::Io { path, source }
    }
}
