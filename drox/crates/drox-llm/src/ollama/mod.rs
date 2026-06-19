//! Provider Ollama (`/api/chat` NDJSON streaming).

mod protocol;
mod stream;

pub use stream::OllamaClient;
