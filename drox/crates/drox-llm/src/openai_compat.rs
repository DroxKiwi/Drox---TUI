//! Liste des modèles via l'API OpenAI-compatible (`GET /v1/models`).

use reqwest::Client;
use url::Url;

use crate::config::LlmConfig;
use crate::error::LlmError;

/// Réponse minimale `GET /v1/models`.
#[derive(Debug, serde::Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct ModelEntry {
    id: String,
}

fn models_url(base: &Url) -> Result<Url, LlmError> {
    let path = base.path().trim_end_matches('/');
    let joined = if path.ends_with("/v1") {
        format!("{path}/models")
    } else if path.is_empty() || path == "/" {
        "/v1/models".to_string()
    } else {
        format!("{path}/v1/models")
    };
    base.join(&joined.trim_start_matches('/'))
        .map_err(LlmError::from)
}

/// Liste les modèles exposés par un serveur OpenAI-compatible.
pub async fn list_models(config: &LlmConfig) -> Result<Vec<String>, LlmError> {
    let url = models_url(&config.base_url)?;
    let client = Client::builder()
        .timeout(config.timeout())
        .build()
        .map_err(LlmError::from)?;
    let mut req = client.get(url);
    for (name, value) in &config.headers {
        req = req.header(name.as_str(), value.as_str());
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(LlmError::Api { status, body });
    }
    let parsed: ModelsResponse = resp.json().await?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_url_appends_v1_when_missing() {
        let cfg = LlmConfig::try_from_str("http://localhost:8000", "m").unwrap();
        let url = models_url(&cfg.base_url).unwrap();
        assert_eq!(url.as_str(), "http://localhost:8000/v1/models");
    }

    #[test]
    fn models_url_keeps_existing_v1_suffix() {
        let cfg = LlmConfig::try_from_str("http://localhost:8000/v1", "m").unwrap();
        let url = models_url(&cfg.base_url).unwrap();
        assert_eq!(url.as_str(), "http://localhost:8000/v1/models");
    }

    #[test]
    fn list_models_parses_response() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/v1/models")
            .with_status(200)
            .with_body(r#"{"data":[{"id":"llama"},{"id":"qwen"}]}"#)
            .create();

        let cfg = LlmConfig::try_from_str(&server.url(), "probe").unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(list_models(&cfg)).unwrap();
        mock.assert();
        assert_eq!(models, vec!["llama", "qwen"]);
    }

    #[test]
    fn list_models_forwards_headers() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/v1/models")
            .match_header("authorization", "Bearer tok")
            .with_status(200)
            .with_body(r#"{"data":[{"id":"m1"}]}"#)
            .create();

        let cfg = LlmConfig::try_from_str(&server.url(), "probe")
            .unwrap()
            .with_header("Authorization", "Bearer tok");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(list_models(&cfg)).unwrap();
        mock.assert();
        assert_eq!(models, vec!["m1"]);
    }
}
