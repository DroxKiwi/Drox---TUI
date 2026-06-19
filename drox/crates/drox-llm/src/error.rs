//! Hiérarchie d'erreurs du client LLM.

use thiserror::Error;

/// Erreurs renvoyées par les implémentations de `LlmClient`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LlmError {
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("LLM API error (status {status}): {body}")]
    Api { status: u16, body: String },

    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("stream terminated unexpectedly")]
    StreamTerminated,

    #[error("stream I/O error: {0}")]
    StreamIo(#[from] std::io::Error),

    #[error("retry exhausted after {attempts} attempts: {source}")]
    RetryExhausted {
        attempts: u32,
        #[source]
        source: Box<Self>,
    },

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("invalid HTTP header: {0}")]
    InvalidHeader(String),
}

impl LlmError {
    /// Indique si l'erreur justifie un retry (réseau transient, etc.).
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(err) => {
                err.is_timeout()
                    || err.is_connect()
                    || err.is_request()
                    || err
                        .status()
                        .is_some_and(|s| s.is_server_error() || s.as_u16() == 429)
            }
            Self::StreamTerminated | Self::StreamIo(_) => true,
            Self::Api { status, .. } => *status >= 500 || *status == 429,
            Self::Json(_)
            | Self::InvalidUrl(_)
            | Self::InvalidConfig(_)
            | Self::InvalidHeader(_)
            | Self::RetryExhausted { .. } => false,
        }
    }
}
