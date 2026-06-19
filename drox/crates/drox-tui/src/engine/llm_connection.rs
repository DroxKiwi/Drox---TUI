//! Connexion serveur IA — test et liste des modèles.

use drox_llm::{LlmConfig, LlmError, OllamaClient};

/// Teste la connexion Ollama et retourne les modèles installés.
pub async fn probe_ollama(server: &str, api_key: Option<&str>) -> Result<Vec<String>, LlmError> {
    let mut config = LlmConfig::try_from_str(server, "probe")?;
    if let Some(key) = api_key {
        if !key.trim().is_empty() {
            config = config.with_api_key(key);
        }
    }
    let client = OllamaClient::new(config)?;
    client.list_installed_models().await
}