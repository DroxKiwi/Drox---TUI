//! Trait `LlmClient` et options de chat.

use async_trait::async_trait;
use drox_types::{Message, StreamEvent};
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::LlmError;

/// Spécification d'un tool exposé au modèle (nom, description, schéma JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema (draft-07 subset) des arguments d'entrée.
    pub parameters: Value,
}

/// Options paramétrant un appel `chat` vers le LLM.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatOptions {
    /// Température de sampling.
    pub temperature: Option<f32>,
    /// Nombre maximum de tokens à générer.
    pub max_tokens: Option<u32>,
    /// Séquences provoquant l'arrêt anticipé.
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    /// Liste des tools disponibles pour ce tour.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    /// Ollama — champ top-level `think` : `true` active le trace *thinking*
    /// pour les modèles compatibles ; `false` le désactive explicitement.
    /// `None` = ne pas envoyer la clé (défaut serveur / modèle).
    #[serde(default)]
    pub think: Option<bool>,
}

impl ChatOptions {
    #[must_use]
    pub const fn with_temperature(mut self, t: f32) -> Self {
        self.temperature = Some(t);
        self
    }

    #[must_use]
    pub const fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    #[must_use]
    pub const fn with_think(mut self, think: Option<bool>) -> Self {
        self.think = think;
        self
    }
}

/// Stream typé d'événements LLM.
pub type StreamHandle = BoxStream<'static, Result<StreamEvent, LlmError>>;

/// Contrat d'un provider LLM.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Lance un appel chat streaming et retourne un flux d'événements.
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<StreamHandle, LlmError>;
}
