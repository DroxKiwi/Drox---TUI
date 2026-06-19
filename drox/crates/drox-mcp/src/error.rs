//! Erreurs de la couche MCP Drox.

use rmcp::service::{ClientInitializeError, ServiceError};
use thiserror::Error;

/// Erreur renvoyée par le chargement de config ou les connexions MCP.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpError {
    #[error("configuration: {0}")]
    Config(String),

    #[error("variable d'environnement manquante: {0}")]
    MissingEnvVar(String),

    #[error("serveur MCP inconnu: {0}")]
    UnknownServer(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("initialisation client MCP: {0}")]
    ClientInit(String),

    #[error("appel MCP: {0}")]
    Service(String),

    #[error("en-tête HTTP invalide: {0}")]
    InvalidHeader(String),
}

impl McpError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn invalid_header(msg: impl Into<String>) -> Self {
        Self::InvalidHeader(msg.into())
    }
}

impl From<ClientInitializeError> for McpError {
    fn from(e: ClientInitializeError) -> Self {
        Self::ClientInit(e.to_string())
    }
}

impl From<ServiceError> for McpError {
    fn from(e: ServiceError) -> Self {
        Self::Service(e.to_string())
    }
}
