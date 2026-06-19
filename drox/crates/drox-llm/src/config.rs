//! Configuration d'un client LLM.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::LlmError;

/// Configuration commune à tous les providers LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// URL de base du serveur LLM (ex: `http://localhost:11434` pour Ollama).
    pub base_url: Url,

    /// Modèle à utiliser côté serveur (ex: `llama3.2`, `qwen2.5-coder:32b`).
    pub model: String,

    /// Timeout par requête HTTP (en secondes). Ne s'applique pas au stream
    /// global : seulement à l'établissement de la connexion et à chaque
    /// chunk reçu.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Nombre maximum de tentatives sur erreurs transitoires.
    #[serde(default = "default_retry_max")]
    pub retry_max: u32,

    /// Headers HTTP additionnels injectés sur chaque requête (par exemple
    /// `x-api-key: sk-…` derrière un reverse-proxy / API gateway). Les noms
    /// sont insensibles à la casse côté HTTP, mais conservés tels quels en
    /// configuration pour faciliter la lecture.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,

    /// Borne de tokens **générés** par tour (alias Ollama `num_predict`). Si
    /// `ChatOptions::max_tokens` est explicitement défini, il prime. Sinon on
    /// utilise cette valeur : Ollama par défaut limite à 128 sur certains
    /// modèles, ce qui coupe les réponses + tool calls.
    #[serde(default = "default_num_predict")]
    pub num_predict: i64,

    /// Taille de la **fenêtre de contexte** Ollama (`num_ctx`).
    ///
    /// Le défaut Ollama est 2048, beaucoup trop petit pour Drox (system
    /// prompt + tools + historique font facilement plus de 2k tokens). On
    /// force une valeur saine pour éviter la troncature silencieuse.
    #[serde(default = "default_num_ctx")]
    pub num_ctx: i64,

    /// `top_p` (nucleus sampling), `None` = laisser Ollama décider (0.9 par
    /// défaut côté Ollama).
    #[serde(default)]
    pub top_p: Option<f32>,

    /// `top_k`, `None` = défaut Ollama (40).
    #[serde(default)]
    pub top_k: Option<i64>,

    /// `repeat_penalty`, `None` = défaut Ollama (1.1).
    #[serde(default)]
    pub repeat_penalty: Option<f32>,

    /// `seed` déterministe pour le sampling. `None` = aléatoire (default).
    #[serde(default)]
    pub seed: Option<i64>,

    /// `min_p` (Ollama ≥ 0.1.30) — filtre les tokens dont la probabilité est
    /// < `min_p * p_max`. Plage utile : `0.0 – 0.1`. Très efficace combiné à
    /// `top_p` pour exclure proprement les tokens improbables sans nuire à la
    /// diversité. `None` = inactif côté serveur.
    #[serde(default)]
    pub min_p: Option<f32>,

    /// `presence_penalty` (style `OpenAI`, supporté par Ollama). Pénalise les
    /// tokens **déjà apparus**, indépendamment de la fréquence. Plage
    /// usuelle : `-2.0 – 2.0`. `None` = inactif. À distinguer de
    /// `repeat_penalty` qui agit multiplicativement sur les logits.
    #[serde(default)]
    pub presence_penalty: Option<f32>,

    /// `frequency_penalty` (style `OpenAI`). Pénalise **proportionnellement**
    /// à la fréquence d'apparition. Plage usuelle : `-2.0 – 2.0`.
    /// Complémentaire de `presence_penalty`. `None` = inactif.
    #[serde(default)]
    pub frequency_penalty: Option<f32>,

    /// `keep_alive` — durée pendant laquelle Ollama garde le modèle chargé
    /// en VRAM après la réponse. Accepte un littéral Ollama : `"5m"`,
    /// `"1h"`, `"0"` (décharge immédiate), `"-1"` (infini), ou un nombre de
    /// secondes en string. `None` = défaut Ollama (~5 minutes).
    #[serde(default)]
    pub keep_alive: Option<String>,
}

const fn default_timeout_secs() -> u64 {
    300
}

const fn default_retry_max() -> u32 {
    3
}

const fn default_num_predict() -> i64 {
    4096
}

const fn default_num_ctx() -> i64 {
    32_768
}

impl LlmConfig {
    pub fn new(base_url: Url, model: impl Into<String>) -> Self {
        Self {
            base_url,
            model: model.into(),
            timeout_secs: default_timeout_secs(),
            retry_max: default_retry_max(),
            headers: BTreeMap::new(),
            num_predict: default_num_predict(),
            num_ctx: default_num_ctx(),
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            seed: None,
            min_p: None,
            presence_penalty: None,
            frequency_penalty: None,
            keep_alive: None,
        }
    }

    /// Surcharge `num_predict` (tokens générés max par tour). 0 ou négatif
    /// laissera Ollama décider (déconseillé).
    #[must_use]
    pub const fn with_num_predict(mut self, n: i64) -> Self {
        self.num_predict = n;
        self
    }

    /// Surcharge `num_ctx` (taille de fenêtre Ollama). Doit être >= taille
    /// effective de la conversation.
    #[must_use]
    pub const fn with_num_ctx(mut self, n: i64) -> Self {
        self.num_ctx = n;
        self
    }

    /// Surcharge `top_p` (sampling nucleus). `None` = défaut Ollama.
    #[must_use]
    pub const fn with_top_p(mut self, v: Option<f32>) -> Self {
        self.top_p = v;
        self
    }

    /// Surcharge `top_k`. `None` = défaut Ollama.
    #[must_use]
    pub const fn with_top_k(mut self, v: Option<i64>) -> Self {
        self.top_k = v;
        self
    }

    /// Surcharge `repeat_penalty`. `None` = défaut Ollama.
    #[must_use]
    pub const fn with_repeat_penalty(mut self, v: Option<f32>) -> Self {
        self.repeat_penalty = v;
        self
    }

    /// Surcharge `seed` (déterminisme du sampling). `None` = aléatoire.
    #[must_use]
    pub const fn with_seed(mut self, v: Option<i64>) -> Self {
        self.seed = v;
        self
    }

    /// Surcharge `min_p` (filtre par probabilité relative au token le plus
    /// probable). `None` = inactif côté Ollama.
    #[must_use]
    pub const fn with_min_p(mut self, v: Option<f32>) -> Self {
        self.min_p = v;
        self
    }

    /// Surcharge `presence_penalty` (style `OpenAI`). `None` = inactif.
    #[must_use]
    pub const fn with_presence_penalty(mut self, v: Option<f32>) -> Self {
        self.presence_penalty = v;
        self
    }

    /// Surcharge `frequency_penalty` (style `OpenAI`). `None` = inactif.
    #[must_use]
    pub const fn with_frequency_penalty(mut self, v: Option<f32>) -> Self {
        self.frequency_penalty = v;
        self
    }

    /// Surcharge `keep_alive` (durée de cache modèle côté Ollama). Accepte
    /// `"5m"`, `"1h"`, `"0"`, `"-1"`, etc. `None` = défaut Ollama.
    #[must_use]
    pub fn with_keep_alive(mut self, v: Option<String>) -> Self {
        self.keep_alive = v;
        self
    }

    pub fn try_from_str(base_url: &str, model: impl Into<String>) -> Result<Self, LlmError> {
        let url = Url::parse(base_url)?;
        Ok(Self::new(url, model))
    }

    pub const fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    /// Ajoute un header HTTP. Le nom est normalisé en lowercase pour éviter
    /// les doublons (`X-API-Key` et `x-api-key`).
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(name.into().to_lowercase(), value.into());
        self
    }

    /// Raccourci pour `with_header("x-api-key", key)`.
    #[must_use]
    pub fn with_api_key(self, key: impl Into<String>) -> Self {
        self.with_header("x-api-key", key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_header_normalizes_name_to_lowercase() {
        let cfg = LlmConfig::try_from_str("http://localhost:11434", "llama3.2")
            .unwrap()
            .with_header("X-API-Key", "sk-abc");
        assert_eq!(
            cfg.headers.get("x-api-key").map(String::as_str),
            Some("sk-abc")
        );
    }

    #[test]
    fn with_api_key_sets_x_api_key_header() {
        let cfg = LlmConfig::try_from_str("http://localhost:11434", "llama3.2")
            .unwrap()
            .with_api_key("sk-xyz");
        assert_eq!(
            cfg.headers.get("x-api-key").map(String::as_str),
            Some("sk-xyz")
        );
    }

    #[test]
    fn headers_round_trip_through_serde() {
        let cfg = LlmConfig::try_from_str("https://gw.example.com", "granite4.1:8b")
            .unwrap()
            .with_api_key("sk-1")
            .with_header("Authorization", "Bearer t");
        let j = serde_json::to_string(&cfg).unwrap();
        let back: LlmConfig = serde_json::from_str(&j).unwrap();
        assert_eq!(back.headers.len(), 2);
        assert_eq!(
            back.headers.get("authorization").map(String::as_str),
            Some("Bearer t")
        );
    }

    #[test]
    fn headers_field_is_omitted_when_empty() {
        let cfg = LlmConfig::try_from_str("http://x", "m").unwrap();
        let j = serde_json::to_string(&cfg).unwrap();
        assert!(!j.contains("headers"), "got: {j}");
    }
}
