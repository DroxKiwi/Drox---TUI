//! Bibliothèque de profils de connexion LLM (presets + custom).
//!
//! Persistance : `TuiPreferences.connection_profiles` + `active_profile_id`.

use std::collections::BTreeMap;

use drox_llm::{LlmConfig, LlmError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::preferences::{
    default_max_iterations_pref, default_num_ctx_pref, is_valid_saved_connection,
    LlmConnectionPrefs, LlmEngineKind, LLM_BOOT_PLACEHOLDER_MODEL, OLLAMA_DEFAULT_SERVER,
};

/// Identifiants des presets intégrés (non supprimables).
pub const PRESET_OLLAMA_LOCAL: &str = "ollama-local";
pub const PRESET_OLLAMA_CLOUD: &str = "ollama-cloud";
pub const PRESET_VLLM_OPENAI: &str = "vllm-openai";
pub const PRESET_LM_STUDIO: &str = "lm-studio";
pub const PRESET_OPENAI_COMPAT: &str = "openai-compatible";

/// Fournisseur / style d'API LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    #[default]
    OllamaLocal,
    OllamaCloud,
    Vllm,
    OpenAiCompatible,
    LmStudio,
    Custom,
}

impl LlmProvider {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OllamaLocal => "Ollama local",
            Self::OllamaCloud => "Ollama Cloud",
            Self::Vllm => "vLLM (OpenAI)",
            Self::OpenAiCompatible => "OpenAI-compatible",
            Self::LmStudio => "LM Studio",
            Self::Custom => "Personnalisé",
        }
    }

    #[must_use]
    pub const fn uses_ollama_api(self) -> bool {
        matches!(self, Self::OllamaLocal | Self::OllamaCloud | Self::Custom)
    }

    #[must_use]
    pub const fn uses_openai_api(self) -> bool {
        matches!(
            self,
            Self::Vllm | Self::OpenAiCompatible | Self::LmStudio
        )
    }
}

/// Authentification HTTP pour un profil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthConfig {
    #[default]
    None,
    Bearer {
        token: String,
    },
    ApiKeyHeader {
        header_name: String,
        token: String,
    },
    /// Headers entièrement définis dans `extra_headers` (pas d'injection auto).
    CustomHeaders,
}

impl AuthConfig {
    #[must_use]
    pub fn as_legacy_api_key(&self) -> Option<String> {
        match self {
            Self::Bearer { token } if !token.trim().is_empty() => Some(token.clone()),
            Self::ApiKeyHeader { token, .. } if !token.trim().is_empty() => Some(token.clone()),
            _ => None,
        }
    }
}

/// Profil de connexion persisté.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub provider: LlmProvider,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_headers: BTreeMap<String, String>,
    #[serde(default = "default_num_ctx_pref")]
    pub num_ctx: i64,
    #[serde(default = "default_max_iterations_pref")]
    pub max_iterations: usize,
    /// Preset intégré (`ollama-local`, …) — non supprimable.
    #[serde(default)]
    pub built_in: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_origin: Option<String>,
    /// Connexion testée avec succès (liste modèles en cache).
    #[serde(default)]
    pub connection_verified: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified_models: Vec<String>,
}

impl ConnectionProfile {
    #[must_use]
    pub fn new_custom(name: impl Into<String>, provider: LlmProvider, base_url: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            provider,
            base_url: base_url.into(),
            default_model: None,
            auth: AuthConfig::None,
            extra_headers: BTreeMap::new(),
            num_ctx: default_num_ctx_pref(),
            max_iterations: default_max_iterations_pref(),
            built_in: false,
            preset_origin: None,
            connection_verified: false,
            verified_models: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_connection_verified(&self) -> bool {
        self.connection_verified && !self.base_url.trim().is_empty()
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        !self.base_url.trim().is_empty()
            && self
                .default_model
                .as_ref()
                .is_some_and(|m| !m.trim().is_empty() && m != LLM_BOOT_PLACEHOLDER_MODEL)
    }

    #[must_use]
    pub fn effective_model(&self) -> Option<&str> {
        self.default_model
            .as_deref()
            .filter(|m| !m.trim().is_empty() && *m != LLM_BOOT_PLACEHOLDER_MODEL)
    }
}

/// Bibliothèque de profils utilisateur + preset actif.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ConnectionLibrary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<ConnectionProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
}

impl ConnectionLibrary {
    pub fn ensure_builtin_presets(&mut self) {
        for template in builtin_preset_templates() {
            if !self.profiles.iter().any(|p| p.id == template.id) {
                self.profiles.push(template);
            }
        }
    }

    #[must_use]
    pub fn active_profile(&self) -> Option<&ConnectionProfile> {
        let id = self.active_profile_id.as_ref()?;
        self.profiles.iter().find(|p| &p.id == id)
    }

    #[must_use]
    pub fn profile_by_id(&self, id: &str) -> Option<&ConnectionProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn set_active(&mut self, id: &str) -> Result<(), &'static str> {
        if self.profiles.iter().any(|p| p.id == id) {
            self.active_profile_id = Some(id.to_string());
            Ok(())
        } else {
            Err("profil introuvable")
        }
    }

    pub fn upsert_profile(&mut self, profile: ConnectionProfile) {
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
    }

    pub fn remove_profile(&mut self, id: &str) -> Result<(), &'static str> {
        let Some(pos) = self.profiles.iter().position(|p| p.id == id) else {
            return Err("profil introuvable");
        };
        if self.profiles[pos].built_in {
            return Err("preset integre non supprimable");
        }
        self.profiles.remove(pos);
        if self.active_profile_id.as_deref() == Some(id) {
            self.active_profile_id = None;
        }
        Ok(())
    }

    /// Met à jour le profil actif (ou en crée un) depuis les champs legacy du modal `/server`.
    pub fn sync_from_legacy_fields(
        &mut self,
        server: &str,
        api_key: Option<&str>,
        model: &str,
        num_ctx: i64,
        max_iterations: usize,
    ) {
        let provider = infer_provider_from_url(server);
        let auth = legacy_api_key_to_auth(api_key);

        if let Some(id) = self.active_profile_id.clone() {
            if let Some(profile) = self.profiles.iter_mut().find(|p| p.id == id) {
                profile.base_url = server.trim().to_string();
                profile.auth = auth;
                profile.default_model = Some(model.to_string());
                profile.num_ctx = num_ctx;
                profile.max_iterations = max_iterations;
                profile.provider = provider;
                return;
            }
        }

        let mut profile = ConnectionProfile::new_custom("Connexion active", provider, server);
        profile.auth = auth;
        profile.default_model = Some(model.to_string());
        profile.num_ctx = num_ctx;
        profile.max_iterations = max_iterations;
        let id = profile.id.clone();
        self.profiles.push(profile);
        self.active_profile_id = Some(id);
    }
}

#[must_use]
pub fn builtin_preset_templates() -> Vec<ConnectionProfile> {
    vec![
        preset(
            PRESET_OLLAMA_LOCAL,
            "Ollama local",
            LlmProvider::OllamaLocal,
            OLLAMA_DEFAULT_SERVER,
            AuthConfig::None,
        ),
        preset(
            PRESET_OLLAMA_CLOUD,
            "Ollama Cloud",
            LlmProvider::OllamaCloud,
            "https://ollama.com",
            AuthConfig::Bearer {
                token: String::new(),
            },
        ),
        preset(
            PRESET_VLLM_OPENAI,
            "vLLM (OpenAI)",
            LlmProvider::Vllm,
            "http://127.0.0.1:8000/v1",
            AuthConfig::Bearer {
                token: String::new(),
            },
        ),
        preset(
            PRESET_LM_STUDIO,
            "LM Studio",
            LlmProvider::LmStudio,
            "http://127.0.0.1:1234/v1",
            AuthConfig::None,
        ),
        preset(
            PRESET_OPENAI_COMPAT,
            "OpenAI-compatible",
            LlmProvider::OpenAiCompatible,
            "http://127.0.0.1:8080/v1",
            AuthConfig::Bearer {
                token: String::new(),
            },
        ),
    ]
}

fn preset(
    id: &str,
    name: &str,
    provider: LlmProvider,
    base_url: &str,
    auth: AuthConfig,
) -> ConnectionProfile {
    ConnectionProfile {
        id: id.to_string(),
        name: name.to_string(),
        provider,
        base_url: base_url.to_string(),
        default_model: None,
        auth,
        extra_headers: BTreeMap::new(),
        num_ctx: default_num_ctx_pref(),
        max_iterations: default_max_iterations_pref(),
        built_in: true,
        preset_origin: Some(id.to_string()),
        connection_verified: false,
        verified_models: Vec::new(),
    }
}

#[must_use]
pub fn infer_provider_from_url(url: &str) -> LlmProvider {
    let lower = url.to_ascii_lowercase();
    if lower.contains("ollama.com") {
        LlmProvider::OllamaCloud
    } else if lower.contains(":1234") || lower.contains("lmstudio") {
        LlmProvider::LmStudio
    } else if lower.contains(":8000") || lower.contains("vllm") {
        LlmProvider::Vllm
    } else if lower.contains("/v1") {
        LlmProvider::OpenAiCompatible
    } else if lower.contains("localhost") || lower.contains("127.0.0.1") {
        LlmProvider::OllamaLocal
    } else {
        LlmProvider::Custom
    }
}

pub(crate) fn legacy_api_key_to_auth(api_key: Option<&str>) -> AuthConfig {
    match api_key.filter(|k| !k.trim().is_empty()) {
        Some(token) => AuthConfig::ApiKeyHeader {
            header_name: "x-api-key".into(),
            token: token.to_string(),
        },
        None => AuthConfig::None,
    }
}

/// Convertit un profil en [`LlmConfig`] (headers auth + extra).
pub fn profile_to_llm_config(profile: &ConnectionProfile) -> Result<LlmConfig, LlmError> {
    let model = profile
        .effective_model()
        .unwrap_or("probe")
        .to_string();
    let mut config = LlmConfig::try_from_str(profile.base_url.trim(), model)?;
    config = config.with_num_ctx(profile.num_ctx.max(2048));
    apply_auth_to_config(&mut config, &profile.auth, &profile.extra_headers);
    Ok(config)
}

/// Construit un [`LlmConfig`] de test (modèle `probe`).
pub fn profile_to_probe_config(profile: &ConnectionProfile) -> Result<LlmConfig, LlmError> {
    let mut config = LlmConfig::try_from_str(profile.base_url.trim(), "probe")?;
    apply_auth_to_config(&mut config, &profile.auth, &profile.extra_headers);
    Ok(config)
}

fn apply_auth_to_config(
    config: &mut LlmConfig,
    auth: &AuthConfig,
    extra_headers: &BTreeMap<String, String>,
) {
    match auth {
        AuthConfig::None => {}
        AuthConfig::Bearer { token } if !token.trim().is_empty() => {
            *config = config.clone().with_header("authorization", format!("Bearer {}", token.trim()));
        }
        AuthConfig::ApiKeyHeader { header_name, token } if !token.trim().is_empty() => {
            *config = config.clone().with_header(header_name.trim(), token.trim());
        }
        AuthConfig::Bearer { .. } | AuthConfig::ApiKeyHeader { .. } => {}
        AuthConfig::CustomHeaders => {}
    }
    for (name, value) in extra_headers {
        if !value.trim().is_empty() {
            *config = config.clone().with_header(name, value);
        }
    }
}

#[must_use]
pub fn profile_to_legacy_prefs(profile: &ConnectionProfile) -> Option<LlmConnectionPrefs> {
    if !profile.is_usable() {
        return None;
    }
    Some(LlmConnectionPrefs {
        engine: LlmEngineKind::Ollama,
        server: profile.base_url.clone(),
        api_key: legacy_api_key_from_profile(profile),
        model: profile.default_model.clone().unwrap_or_default(),
        num_ctx: profile.num_ctx,
        max_iterations: profile.max_iterations,
    })
}

fn legacy_api_key_from_profile(profile: &ConnectionProfile) -> Option<String> {
    match &profile.auth {
        AuthConfig::Bearer { token } if !token.trim().is_empty() => Some(token.clone()),
        AuthConfig::ApiKeyHeader { token, .. } if !token.trim().is_empty() => Some(token.clone()),
        _ => profile
            .extra_headers
            .get("x-api-key")
            .cloned()
            .filter(|k| !k.trim().is_empty()),
    }
}

#[must_use]
pub fn legacy_prefs_to_profile(conn: &LlmConnectionPrefs, name: &str) -> ConnectionProfile {
    let provider = infer_provider_from_url(&conn.server);
    let mut profile = ConnectionProfile::new_custom(name, provider, &conn.server);
    profile.auth = legacy_api_key_to_auth(conn.api_key.as_deref());
    profile.default_model = Some(conn.model.clone());
    profile.num_ctx = conn.num_ctx;
    profile.max_iterations = conn.max_iterations;
    profile.connection_verified = is_valid_saved_connection(conn);
    profile
}

/// Migration : prefs 2.0.1 → bibliothèque 2.0.2.
#[must_use]
pub fn migrate_library_from_legacy(
    library: ConnectionLibrary,
    legacy: Option<&LlmConnectionPrefs>,
) -> ConnectionLibrary {
    let mut library = library;
    library.ensure_builtin_presets();
    if library.active_profile_id.is_some() {
        return library;
    }
    if let Some(conn) = legacy.filter(|c| is_valid_saved_connection(c)) {
        let profile = legacy_prefs_to_profile(conn, "Import 2.0.1");
        let id = profile.id.clone();
        library.profiles.push(profile);
        library.active_profile_id = Some(id);
    }
    library
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_presets_present_after_ensure() {
        let mut lib = ConnectionLibrary::default();
        lib.ensure_builtin_presets();
        assert!(lib.profile_by_id(PRESET_OLLAMA_LOCAL).is_some());
        assert!(lib.profile_by_id(PRESET_OLLAMA_CLOUD).is_some());
        assert_eq!(lib.profiles.len(), 5);
    }

    #[test]
    fn bearer_auth_sets_authorization_header() {
        let mut profile = ConnectionProfile::new_custom("cloud", LlmProvider::OllamaCloud, "https://ollama.com");
        profile.auth = AuthConfig::Bearer {
            token: "tok123".into(),
        };
        profile.default_model = Some("llama3.2".into());
        let cfg = profile_to_llm_config(&profile).unwrap();
        assert_eq!(
            cfg.headers.get("authorization").map(String::as_str),
            Some("Bearer tok123")
        );
    }

    #[test]
    fn api_key_header_custom_name() {
        let mut profile =
            ConnectionProfile::new_custom("perso", LlmProvider::Custom, "https://gw.example.com");
        profile.auth = AuthConfig::ApiKeyHeader {
            header_name: "X-Ollama-Token".into(),
            token: "secret".into(),
        };
        profile.default_model = Some("qwen".into());
        let cfg = profile_to_llm_config(&profile).unwrap();
        assert_eq!(
            cfg.headers.get("x-ollama-token").map(String::as_str),
            Some("secret")
        );
    }

    #[test]
    fn extra_headers_merge_with_auth() {
        let mut profile =
            ConnectionProfile::new_custom("gw", LlmProvider::Custom, "https://gw.example.com");
        profile.auth = AuthConfig::CustomHeaders;
        profile
            .extra_headers
            .insert("X-Custom".into(), "v1".into());
        profile.default_model = Some("m".into());
        let cfg = profile_to_llm_config(&profile).unwrap();
        assert_eq!(cfg.headers.get("x-custom").map(String::as_str), Some("v1"));
    }

    #[test]
    fn legacy_round_trip() {
        let legacy = LlmConnectionPrefs {
            engine: LlmEngineKind::Ollama,
            server: "http://127.0.0.1:11434".into(),
            api_key: Some("key".into()),
            model: "qwen".into(),
            num_ctx: 32_768,
            max_iterations: 24,
        };
        let profile = legacy_prefs_to_profile(&legacy, "test");
        let back = profile_to_legacy_prefs(&profile).unwrap();
        assert_eq!(back.server, legacy.server);
        assert_eq!(back.model, legacy.model);
        assert_eq!(back.api_key, legacy.api_key);
    }

    #[test]
    fn migrate_from_legacy_sets_active() {
        let legacy = LlmConnectionPrefs {
            engine: LlmEngineKind::Ollama,
            server: "http://127.0.0.1:11434".into(),
            api_key: None,
            model: "qwen".into(),
            num_ctx: 32_768,
            max_iterations: 12,
        };
        let lib = migrate_library_from_legacy(ConnectionLibrary::default(), Some(&legacy));
        assert!(lib.active_profile().is_some());
        assert_eq!(lib.active_profile().unwrap().default_model.as_deref(), Some("qwen"));
    }

    #[test]
    fn cannot_delete_builtin_preset() {
        let mut lib = ConnectionLibrary::default();
        lib.ensure_builtin_presets();
        assert!(lib.remove_profile(PRESET_OLLAMA_LOCAL).is_err());
    }
}
