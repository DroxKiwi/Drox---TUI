//! `drox-llm` — client LLM unifié.
//!
//! Streaming SSE/NDJSON, retry/backoff, abstraction provider. Ollama est le
//! provider de référence en Phase 1 ; d'autres pourront s'ajouter derrière le
//! même trait.
//!
//! ## Exemple d'utilisation
//!
//! ```no_run
//! # async fn demo() -> Result<(), drox_llm::LlmError> {
//! use drox_llm::{ChatOptions, LlmClient, LlmConfig, OllamaClient};
//! use drox_types::Message;
//! use futures::StreamExt;
//!
//! let config = LlmConfig::try_from_str("http://localhost:11434", "llama3.2")?;
//! let client = OllamaClient::new(config)?;
//! let messages = vec![Message::user("bonjour")];
//! let mut stream = client.stream_chat(messages, ChatOptions::default()).await?;
//! while let Some(event) = stream.next().await {
//!     println!("{:?}", event?);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Voir `docs/INVENTAIRE-NOYAU-MOTEUR.md` § 2.2.

pub mod client;
pub mod config;
pub mod error;
pub mod ollama;
pub mod retry;

pub use client::{ChatOptions, LlmClient, StreamHandle, ToolSpec};
pub use config::LlmConfig;
pub use error::LlmError;
pub use ollama::OllamaClient;
pub use retry::with_retry;
