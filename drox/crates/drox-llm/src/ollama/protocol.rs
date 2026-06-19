//! Types wire pour le protocole Ollama `/api/chat`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ChatRequestOptions>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ChatToolSpec<'a>>,
    /// Durée pendant laquelle Ollama garde le modèle chargé en VRAM après la
    /// réponse. Top-level (PAS dans `options`). Format Ollama : `"5m"`,
    /// `"1h"`, `"0"` (déchargement immédiat), `"-1"` (infini). `None` =
    /// défaut serveur (généralement 5 minutes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
    /// Active / désactive le flux `message.thinking` (modèles *thinking*).
    /// Top-level, comme documenté par Ollama — pas dans `options`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChatMessage {
    pub role: &'static str,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ChatToolCallWire>,
    /// Images base64 brutes (sans préfixe `data:`), attendues par Ollama pour
    /// les modèles vision (llava, gemma3-vision, qwen-vl, …). Omis si vide.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChatToolCallWire {
    pub function: ChatToolCallFunction,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChatToolCallFunction {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChatToolSpec<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: ChatToolSpecFunction<'a>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChatToolSpecFunction<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub parameters: &'a Value,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct ChatRequestOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Borne supérieure sur les tokens générés en sortie (alias Ollama de
    /// `max_tokens`). On l'envoie par défaut pour ne pas hériter du
    /// `num_predict = 128` parfois utilisé côté Ollama.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i64>,
    /// Taille de la fenêtre de contexte (tokens). Par défaut Ollama : 2048,
    /// trop petit pour Drox. On envoie systématiquement la valeur configurée
    /// pour garantir que le system prompt + le transcript ne sont pas
    /// silencieusement tronqués.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<i64>,
    /// Sampling nucleus. Plage usuelle : `0.5–0.95`. Default Ollama : `0.9`.
    /// Plus bas = sorties plus conservatrices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-K sampling. Plage usuelle : `10–100`. Default Ollama : `40`.
    /// Borne le nombre de tokens candidats à chaque étape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i64>,
    /// Pénalité de répétition. Plage usuelle : `1.0–1.5`. Default Ollama :
    /// `1.1`. `>1` réduit les boucles répétitives ; `<1` les autorise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// Seed déterministe pour le sampling. Si présent, deux runs avec le
    /// même prompt et la même config produisent la même sortie (utile pour
    /// debug / reproductibilité). `-1` = aléatoire (Ollama).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// `min_p` (Ollama ≥ 0.1.30) : tokens dont la proba `< min_p * p_max`
    /// sont coupés. Plage utile : `0.0–0.1`. Combine bien avec `top_p`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,
    /// `presence_penalty` (style `OpenAI`). Pénalité fixe sur tokens déjà vus,
    /// indépendamment de la fréquence. Plage : `-2.0 – 2.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// `frequency_penalty` (style `OpenAI`). Pénalité proportionnelle à la
    /// fréquence d'apparition. Plage : `-2.0 – 2.0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub stop: Vec<String>,
}

/// Une ligne NDJSON renvoyée par Ollama pendant le stream.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct ChatResponseChunk {
    #[serde(default)]
    pub message: Option<ChatResponseMessage>,
    /// Ollama omet parfois `done` sur les lignes intermédiaires (contexte plein, proxy).
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub done_reason: Option<String>,
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
    #[serde(default)]
    pub eval_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ChatResponseMessage {
    #[serde(default)]
    pub content: String,
    /// Trace de raisonnement natif (Ollama *thinking*), streamé en deltas.
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub tool_calls: Vec<ChatResponseToolCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ChatResponseToolCall {
    pub function: ChatResponseToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ChatResponseToolCallFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}
