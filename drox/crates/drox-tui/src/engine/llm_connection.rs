//! Connexion serveur IA — test et liste des modèles.

use drox_llm::{list_openai_compat_models, LlmError, OllamaClient};

use super::connection_library::{profile_to_probe_config, ConnectionProfile, LlmProvider};

/// Message d'erreur probe lisible (401 → token, 404 → URL, …).
#[must_use]
pub fn format_probe_error(err: &LlmError) -> String {
    match err {
        LlmError::Api { status: 401, .. } => {
            "401 — verifier le token / cle API (Bearer ou header attendu)".into()
        }
        LlmError::Api { status: 403, .. } => {
            "403 — acces refuse (droits token ou endpoint)".into()
        }
        LlmError::Api { status: 404, .. } => {
            "404 — URL incorrecte ou endpoint indisponible".into()
        }
        LlmError::Api { status, body } => format!("HTTP {status} — {body}"),
        other => other.to_string(),
    }
}

/// Teste la connexion Ollama et retourne les modèles installés.
pub async fn probe_ollama(server: &str, api_key: Option<&str>) -> Result<Vec<String>, LlmError> {
    let provider = super::connection_library::infer_provider_from_url(server);
    let mut profile = ConnectionProfile::new_custom("probe", provider, server);
    profile.auth = super::connection_library::legacy_api_key_to_auth(api_key, server);
    probe_connection(&profile).await
}

/// Teste un profil de connexion (Ollama natif ou OpenAI-compatible).
pub async fn probe_connection(profile: &ConnectionProfile) -> Result<Vec<String>, LlmError> {
    let config = profile_to_probe_config(profile)?;
    if profile.provider.uses_openai_api() {
        list_openai_compat_models(&config).await
    } else {
        let client = OllamaClient::new(config)?;
        client.list_installed_models().await
    }
}

/// Teste à partir des champs legacy du modal `/server` (compat UI actuelle).
pub async fn probe_legacy_fields(
    server: &str,
    api_key: Option<&str>,
    provider: LlmProvider,
) -> Result<Vec<String>, LlmError> {
    let mut profile = ConnectionProfile::new_custom("probe", provider, server);
    profile.auth = super::connection_library::legacy_api_key_to_auth(api_key, server);
    probe_connection(&profile).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::connection_library::{AuthConfig, PRESET_OLLAMA_LOCAL};

    #[test]
    fn probe_config_from_preset_local() {
        let profile = crate::engine::connection_library::builtin_preset_templates()
            .into_iter()
            .find(|p| p.id == PRESET_OLLAMA_LOCAL)
            .unwrap();
        let cfg = profile_to_probe_config(&profile).unwrap();
        assert_eq!(cfg.base_url.as_str(), "http://localhost:11434/");
    }

    #[test]
    fn openai_provider_uses_openai_flag() {
        let profile = ConnectionProfile::new_custom("v", LlmProvider::Vllm, "http://127.0.0.1:8000/v1");
        assert!(profile.provider.uses_openai_api());
    }

    #[test]
    fn format_probe_error_maps_401() {
        let err = LlmError::Api {
            status: 401,
            body: "unauthorized".into(),
        };
        assert!(format_probe_error(&err).contains("401"));
    }

    #[test]
    fn bearer_profile_probe_config_has_auth_header() {
        let mut profile =
            ConnectionProfile::new_custom("c", LlmProvider::OllamaCloud, "https://ollama.com");
        profile.auth = AuthConfig::Bearer {
            token: "abc".into(),
        };
        let cfg = profile_to_probe_config(&profile).unwrap();
        assert_eq!(
            cfg.headers.get("authorization").map(String::as_str),
            Some("Bearer abc")
        );
    }
}
