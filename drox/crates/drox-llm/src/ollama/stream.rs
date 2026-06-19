//! Implémentation `LlmClient` pour Ollama via `/api/chat`.

use async_trait::async_trait;
use bytes::Bytes;
use drox_types::{Content, Message, Role, StopReason, StreamEvent, ToolUseId, Usage};
use futures::stream::{self, Stream, StreamExt, TryStreamExt};
use reqwest::Client;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader;
use tracing::{debug, instrument};
use url::Url;

use crate::client::{ChatOptions, LlmClient, StreamHandle};
use crate::config::LlmConfig;
use crate::error::LlmError;
use crate::ollama::protocol::{
    ChatMessage, ChatRequest, ChatRequestOptions, ChatResponseChunk, ChatResponseToolCall,
    ChatToolCallFunction, ChatToolCallWire, ChatToolSpec, ChatToolSpecFunction,
};

/// Defaults d'options Ollama appliqués à chaque requête.
///
/// Ces valeurs sont **toujours envoyées au serveur** (sauf si elles sont
/// `None`), pour éviter d'hériter des defaults silencieux d'Ollama qui sont
/// inadaptés à Drox (`num_predict = 128`, `num_ctx = 2048`).
#[derive(Debug, Clone, Copy)]
pub(super) struct OllamaSamplingDefaults {
    pub num_predict: i64,
    pub num_ctx: i64,
    pub top_p: Option<f32>,
    pub top_k: Option<i64>,
    pub repeat_penalty: Option<f32>,
    pub seed: Option<i64>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
}

/// Client Ollama.
pub struct OllamaClient {
    http: Client,
    base_url: Url,
    model: String,
    defaults: OllamaSamplingDefaults,
    keep_alive: Option<String>,
}

impl OllamaClient {
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        let mut builder = Client::builder().timeout(config.timeout());
        if !config.headers.is_empty() {
            let mut hm = reqwest::header::HeaderMap::with_capacity(config.headers.len());
            for (k, v) in &config.headers {
                let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| LlmError::InvalidHeader(format!("name `{k}`: {e}")))?;
                let value = reqwest::header::HeaderValue::from_str(v)
                    .map_err(|e| LlmError::InvalidHeader(format!("value for header `{k}`: {e}")))?;
                hm.insert(name, value);
            }
            builder = builder.default_headers(hm);
        }
        let http = builder.build()?;
        Ok(Self {
            http,
            base_url: config.base_url,
            model: config.model,
            defaults: OllamaSamplingDefaults {
                num_predict: config.num_predict,
                num_ctx: config.num_ctx,
                top_p: config.top_p,
                top_k: config.top_k,
                repeat_penalty: config.repeat_penalty,
                seed: config.seed,
                min_p: config.min_p,
                presence_penalty: config.presence_penalty,
                frequency_penalty: config.frequency_penalty,
            },
            keep_alive: config.keep_alive,
        })
    }

    fn api_url(&self, suffix: &str) -> Result<Url, LlmError> {
        let mut url = self.base_url.clone();
        if !url.path().ends_with('/') {
            let mut path = url.path().to_owned();
            path.push('/');
            url.set_path(&path);
        }
        Ok(url.join(suffix)?)
    }

    fn chat_endpoint(&self) -> Result<Url, LlmError> {
        self.api_url("api/chat")
    }

    /// Modèle configuré pour les requêtes chat.
    #[must_use]
    pub fn configured_model(&self) -> &str {
        &self.model
    }

    /// URL de base du serveur LLM.
    #[must_use]
    pub fn server_url(&self) -> &Url {
        &self.base_url
    }

    /// Liste les modèles installés (`GET /api/tags`, format Ollama).
    pub async fn list_installed_models(&self) -> Result<Vec<String>, LlmError> {
        let url = self.api_url("api/tags")?;
        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api { status, body });
        }
        let parsed: TagsResponse = resp.json().await?;
        Ok(parsed.models.into_iter().map(|m| m.name).collect())
    }
}

#[derive(Debug, serde::Deserialize)]
struct TagsResponse {
    models: Vec<TagEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct TagEntry {
    name: String,
}

#[async_trait]
impl LlmClient for OllamaClient {
    #[instrument(skip(self, messages, options), fields(model = %self.model, msg_count = messages.len(), tools = options.tools.len()))]
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        options: ChatOptions,
    ) -> Result<StreamHandle, LlmError> {
        let url = self.chat_endpoint()?;
        let payload = build_request(
            &self.model,
            &messages,
            &options,
            self.defaults,
            self.keep_alive.as_deref(),
        );
        debug!(%url, "POST Ollama chat");

        let response = self.http.post(url).json(&payload).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let bytes_stream = response.bytes_stream();
        Ok(events_from_ndjson(bytes_stream).boxed())
    }
}

/// Construit le payload Ollama à partir du modèle, des messages et options.
///
/// Les `defaults` (`num_predict`, `num_ctx`, `top_p`, `top_k`,
/// `repeat_penalty`, `seed`) viennent de la config client et sont **toujours
/// envoyés** quand ils sont positionnés, sauf si l'appelant les surcharge
/// via `ChatOptions::max_tokens` (qui prend le pas sur `num_predict`). Ces
/// defaults évitent les troncatures silencieuses (`num_predict = 128`,
/// `num_ctx = 2048`) qui causent l'arrêt prématuré du modèle, et permettent
/// à l'utilisateur de fixer `top_p` / `top_k` / `repeat_penalty` / `seed`
/// sans avoir à toucher au code.
fn build_request<'a>(
    model: &'a str,
    messages: &[Message],
    options: &'a ChatOptions,
    defaults: OllamaSamplingDefaults,
    keep_alive: Option<&str>,
) -> ChatRequest<'a> {
    let chat_messages = messages.iter().map(message_to_wire).collect();

    let num_predict = options
        .max_tokens
        .map(i64::from)
        .or_else(|| (defaults.num_predict > 0).then_some(defaults.num_predict));
    let num_ctx = (defaults.num_ctx > 0).then_some(defaults.num_ctx);

    let has_any_option = options.temperature.is_some()
        || num_predict.is_some()
        || num_ctx.is_some()
        || defaults.top_p.is_some()
        || defaults.top_k.is_some()
        || defaults.repeat_penalty.is_some()
        || defaults.seed.is_some()
        || defaults.min_p.is_some()
        || defaults.presence_penalty.is_some()
        || defaults.frequency_penalty.is_some()
        || !options.stop_sequences.is_empty();
    let opts = has_any_option.then(|| ChatRequestOptions {
        temperature: options.temperature,
        num_predict,
        num_ctx,
        top_p: defaults.top_p,
        top_k: defaults.top_k,
        repeat_penalty: defaults.repeat_penalty,
        seed: defaults.seed,
        min_p: defaults.min_p,
        presence_penalty: defaults.presence_penalty,
        frequency_penalty: defaults.frequency_penalty,
        stop: options.stop_sequences.clone(),
    });

    let tools = options
        .tools
        .iter()
        .map(|t| ChatToolSpec {
            kind: "function",
            function: ChatToolSpecFunction {
                name: &t.name,
                description: &t.description,
                parameters: &t.parameters,
            },
        })
        .collect();

    ChatRequest {
        model,
        messages: chat_messages,
        stream: true,
        options: opts,
        tools,
        keep_alive: keep_alive.map(str::to_owned),
        think: options.think,
    }
}

/// Convertit un message Drox vers son équivalent wire Ollama.
///
/// Stratégie :
/// - `Content::Text` → fusionnés dans `content`.
/// - `Content::ToolUse` (assistant uniquement) → `tool_calls`.
/// - `Content::ToolResult` (rôle tool) → `content = result_content`.
fn message_to_wire(m: &Message) -> ChatMessage {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut images = Vec::new();

    for block in &m.content {
        match block {
            Content::Text { text: t } => text.push_str(t),
            Content::Image { data, .. } => {
                images.push(data.clone());
            }
            Content::ToolUse {
                name, input: args, ..
            } => {
                tool_calls.push(ChatToolCallWire {
                    function: ChatToolCallFunction {
                        name: name.clone(),
                        arguments: args.clone(),
                    },
                });
            }
            Content::ToolResult {
                content: result_content,
                ..
            } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(result_content);
            }
            // `Content` est `#[non_exhaustive]` : nouveaux variants futurs
            // ignorés silencieusement par Ollama (la sérialisation se fera
            // par d'autres providers).
            _ => {}
        }
    }

    ChatMessage {
        role: role_to_str(m.role),
        content: text,
        tool_calls,
        images,
    }
}

const fn role_to_str(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

/// Transforme un stream d'octets HTTP en stream typé d'événements LLM.
fn events_from_ndjson<S>(
    bytes_stream: S,
) -> impl Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    use std::sync::{Arc, Mutex};

    // Tampon des `tool_calls` Ollama : le serveur peut les renvoyer sur
    // plusieurs lignes NDJSON intermédiaires. Si on émettait un
    // `StreamEvent::ToolCall` à chaque ligne, on générerait un **nouveau**
    // `ToolUseId` à chaque fois, ce qui désynchronise `tool_start` /
    // `tool_finish` côté client (plus de diff dans le chat). On ne pousse
    // donc les `ToolCall` qu'une fois, au moment du chunk `done: true`, en
    // se basant sur la dernière copie reçue.
    let tool_calls_buf = Arc::new(Mutex::new(Vec::<ChatResponseToolCall>::new()));

    let io_stream = bytes_stream.map_err(std::io::Error::other);
    let reader = StreamReader::new(io_stream);
    let lines = FramedRead::new(reader, LinesCodec::new());

    let parsed = lines
        .map_err(|e| match e {
            tokio_util::codec::LinesCodecError::Io(io) => LlmError::StreamIo(io),
            tokio_util::codec::LinesCodecError::MaxLineLengthExceeded => LlmError::StreamTerminated,
        })
        .and_then({
            let tool_calls_buf = Arc::clone(&tool_calls_buf);
            move |line: String| {
                let tool_calls_buf = Arc::clone(&tool_calls_buf);
                async move {
                    if line.trim().is_empty() {
                        return Ok(Vec::new());
                    }
                    let chunk: ChatResponseChunk = serde_json::from_str(&line)?;
                    let mut buf = tool_calls_buf
                        .lock()
                        .expect("tool_calls buffer mutex poisoned");
                    Ok(chunk_to_events_buffered(chunk, &mut buf))
                }
            }
        });

    let start = stream::once(async { Ok(StreamEvent::Start) });
    start.chain(parsed.flat_map(|res| match res {
        Ok(events) => stream::iter(events.into_iter().map(Ok)).boxed(),
        Err(err) => stream::iter(std::iter::once(Err(err))).boxed(),
    }))
}

fn chunk_to_events_buffered(
    chunk: ChatResponseChunk,
    tool_calls_buf: &mut Vec<ChatResponseToolCall>,
) -> Vec<StreamEvent> {
    let mut events = Vec::with_capacity(3);

    if let Some(msg) = chunk.message {
        if !msg.thinking.is_empty() {
            events.push(StreamEvent::ThinkingDelta {
                text: msg.thinking,
            });
        }
        if !msg.content.is_empty() {
            events.push(StreamEvent::TextDelta { text: msg.content });
        }
        if !msg.tool_calls.is_empty() {
            *tool_calls_buf = msg.tool_calls;
        }
    }

    if chunk.done {
        for call in tool_calls_buf.drain(..) {
            events.push(StreamEvent::ToolCall {
                id: ToolUseId::new(),
                name: call.function.name,
                arguments: call.function.arguments,
            });
        }
        let reason = map_done_reason(chunk.done_reason.as_deref());
        let usage = Usage {
            input_tokens: chunk.prompt_eval_count.unwrap_or(0),
            output_tokens: chunk.eval_count.unwrap_or(0),
        };
        events.push(StreamEvent::Stop { reason, usage });
    }

    events
}

fn map_done_reason(raw: Option<&str>) -> StopReason {
    match raw {
        Some("length" | "max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        Some("tool_use" | "tool_calls") => StopReason::ToolUse,
        None | Some(_) => StopReason::EndTurn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_accepts_valid_headers() {
        let cfg = LlmConfig::try_from_str("http://localhost:11434", "llama3.2")
            .unwrap()
            .with_api_key("sk-abc");
        assert!(OllamaClient::new(cfg).is_ok());
    }

    #[test]
    fn new_rejects_invalid_header_name() {
        let cfg = LlmConfig::try_from_str("http://localhost:11434", "m")
            .unwrap()
            .with_header("x api key", "v"); // espace interdit dans un nom HTTP
        match OllamaClient::new(cfg) {
            Err(LlmError::InvalidHeader(_)) => {}
            Err(e) => panic!("expected InvalidHeader, got {e:?}"),
            Ok(_) => panic!("expected InvalidHeader, got Ok(_)"),
        }
    }

    /// Helpers de test : defaults Drox (`num_predict=4096`, `num_ctx=32k`) et
    /// defaults "minimal" (zéros = ne rien envoyer).
    fn test_defaults() -> OllamaSamplingDefaults {
        OllamaSamplingDefaults {
            num_predict: 4096,
            num_ctx: 32_768,
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            seed: None,
            min_p: None,
            presence_penalty: None,
            frequency_penalty: None,
        }
    }

    fn empty_defaults() -> OllamaSamplingDefaults {
        OllamaSamplingDefaults {
            num_predict: 0,
            num_ctx: 0,
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            seed: None,
            min_p: None,
            presence_penalty: None,
            frequency_penalty: None,
        }
    }

    #[test]
    fn build_request_applies_defaults_even_when_options_empty() {
        let messages = vec![Message::user("hello")];
        let opts = ChatOptions::default();
        let req = build_request("test-model", &messages, &opts, test_defaults(), None);
        assert_eq!(req.model, "test-model");
        assert!(req.stream);
        let opt = req
            .options
            .expect("defaults num_predict / num_ctx must be sent");
        assert_eq!(opt.num_predict, Some(4096));
        assert_eq!(opt.num_ctx, Some(32_768));
        assert!(opt.temperature.is_none());
        assert!(opt.top_p.is_none());
        assert!(req.tools.is_empty());
        assert!(req.keep_alive.is_none());
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content, "hello");
    }

    #[test]
    fn build_request_max_tokens_overrides_default_num_predict() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default()
            .with_temperature(0.5)
            .with_max_tokens(100);
        let req = build_request("m", &messages, &opts, test_defaults(), None);
        let opt = req.options.expect("options should be Some");
        assert_eq!(opt.temperature, Some(0.5));
        assert_eq!(opt.num_predict, Some(100));
        assert_eq!(opt.num_ctx, Some(32_768));
    }

    #[test]
    fn build_request_disables_defaults_when_zero() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default();
        let req = build_request("m", &messages, &opts, empty_defaults(), None);
        assert!(
            req.options.is_none(),
            "no options should be sent when defaults are disabled"
        );
    }

    #[test]
    fn build_request_forwards_advanced_sampling_options() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default();
        let defaults = OllamaSamplingDefaults {
            num_predict: 4096,
            num_ctx: 32_768,
            top_p: Some(0.85),
            top_k: Some(50),
            repeat_penalty: Some(1.2),
            seed: Some(42),
            min_p: Some(0.05),
            presence_penalty: Some(0.2),
            frequency_penalty: Some(0.1),
        };
        let req = build_request("m", &messages, &opts, defaults, Some("10m"));
        let opt = req.options.expect("options should be sent");
        assert_eq!(opt.top_p, Some(0.85));
        assert_eq!(opt.top_k, Some(50));
        assert_eq!(opt.repeat_penalty, Some(1.2));
        assert_eq!(opt.seed, Some(42));
        assert_eq!(opt.min_p, Some(0.05));
        assert_eq!(opt.presence_penalty, Some(0.2));
        assert_eq!(opt.frequency_penalty, Some(0.1));
        assert_eq!(req.keep_alive.as_deref(), Some("10m"));
    }

    #[test]
    fn build_request_advanced_options_alone_force_options_block() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default();
        let defaults = OllamaSamplingDefaults {
            num_predict: 0,
            num_ctx: 0,
            top_p: Some(0.7),
            top_k: None,
            repeat_penalty: None,
            seed: None,
            min_p: None,
            presence_penalty: None,
            frequency_penalty: None,
        };
        let req = build_request("m", &messages, &opts, defaults, None);
        let opt = req.options.expect("`top_p` only must still emit options");
        assert_eq!(opt.top_p, Some(0.7));
        assert!(opt.num_predict.is_none());
        assert!(opt.num_ctx.is_none());
    }

    #[test]
    fn build_request_emits_min_p_alone() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default();
        let defaults = OllamaSamplingDefaults {
            num_predict: 0,
            num_ctx: 0,
            top_p: None,
            top_k: None,
            repeat_penalty: None,
            seed: None,
            min_p: Some(0.07),
            presence_penalty: None,
            frequency_penalty: None,
        };
        let req = build_request("m", &messages, &opts, defaults, None);
        let opt = req.options.expect("min_p alone must still emit options");
        assert_eq!(opt.min_p, Some(0.07));
    }

    #[test]
    fn build_request_serializes_keep_alive_at_top_level() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default();
        let req = build_request("m", &messages, &opts, test_defaults(), Some("0"));
        let body = serde_json::to_value(&req).expect("serialize");
        assert_eq!(body["keep_alive"], serde_json::json!("0"));
        assert!(
            body["options"].get("keep_alive").is_none(),
            "keep_alive must NOT be nested in options"
        );
    }

    #[test]
    fn build_request_serializes_tool_calls_on_assistant_message() {
        use drox_types::{Content, ToolUseId};

        let id = ToolUseId::new();
        let assistant = Message::new(
            Role::Assistant,
            vec![
                Content::text("Je vais lire le fichier."),
                Content::ToolUse {
                    id,
                    name: "file_read".into(),
                    input: json!({ "path": "Cargo.toml" }),
                },
            ],
        );
        let opts = ChatOptions::default();
        let req = build_request("m", &[assistant], &opts, empty_defaults(), None);
        let m = &req.messages[0];
        assert_eq!(m.role, "assistant");
        assert_eq!(m.content, "Je vais lire le fichier.");
        assert_eq!(m.tool_calls.len(), 1);
        assert_eq!(m.tool_calls[0].function.name, "file_read");
    }

    #[test]
    fn build_request_serializes_user_image_into_images_field() {
        use drox_types::Content;

        let user = Message::user_with_blocks(vec![
            Content::text("regarde:"),
            Content::image("image/png", "AAAA"),
            Content::image("image/jpeg", "BBBB"),
        ]);
        let opts = ChatOptions::default();
        let req = build_request("m", &[user], &opts, empty_defaults(), None);
        let m = &req.messages[0];
        assert_eq!(m.role, "user");
        assert_eq!(m.content, "regarde:");
        assert_eq!(m.images, vec!["AAAA".to_string(), "BBBB".to_string()]);
        let serialized = serde_json::to_value(m).unwrap();
        assert_eq!(serialized["images"][0], "AAAA");
        assert_eq!(serialized["images"][1], "BBBB");
    }

    #[test]
    fn build_request_omits_images_field_when_no_images() {
        let user = Message::user("plain text");
        let opts = ChatOptions::default();
        let req = build_request("m", &[user], &opts, empty_defaults(), None);
        let serialized = serde_json::to_value(&req.messages[0]).unwrap();
        assert!(
            serialized.get("images").is_none(),
            "images field should be omitted when empty"
        );
    }

    #[test]
    fn build_request_serializes_tool_result_into_content() {
        let id = drox_types::ToolUseId::new();
        let tool_msg = Message::tool_result(id, "contenu fichier", false);
        let opts = ChatOptions::default();
        let req = build_request("m", &[tool_msg], &opts, empty_defaults(), None);
        let m = &req.messages[0];
        assert_eq!(m.role, "tool");
        assert_eq!(m.content, "contenu fichier");
    }

    #[test]
    fn chunk_with_thinking_delta_then_content() {
        let chunk: ChatResponseChunk = serde_json::from_value(json!({
            "message": { "role": "assistant", "thinking": "step", "content": "" },
            "done": false,
        }))
        .unwrap();
        let mut buf = Vec::new();
        let events = chunk_to_events_buffered(chunk, &mut buf);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::ThinkingDelta { text } if text == "step"));
    }

    #[test]
    fn build_request_forwards_think_flag() {
        let messages = vec![Message::user("x")];
        let opts = ChatOptions::default().with_think(Some(true));
        let req = build_request("qwen3", &messages, &opts, empty_defaults(), None);
        assert_eq!(req.think, Some(true));
        let body = serde_json::to_value(&req).expect("serialize");
        assert_eq!(body["think"], json!(true));
    }

    #[test]
    fn chunk_with_text_delta_only() {
        let chunk: ChatResponseChunk = serde_json::from_value(json!({
            "message": { "role": "assistant", "content": "hello" },
            "done": false,
        }))
        .unwrap();
        let mut buf = Vec::new();
        let events = chunk_to_events_buffered(chunk, &mut buf);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::TextDelta { text } if text == "hello"));
    }

    #[test]
    fn chunk_with_tool_call_emits_on_done_only() {
        let mut buf = Vec::new();
        let chunk: ChatResponseChunk = serde_json::from_value(json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    { "function": { "name": "file_read", "arguments": { "path": "a.txt" } } }
                ],
            },
            "done": true,
            "done_reason": "tool_calls",
            "prompt_eval_count": 1,
            "eval_count": 2,
        }))
        .unwrap();
        let events = chunk_to_events_buffered(chunk, &mut buf);
        assert_eq!(events.len(), 2);
        match &events[0] {
            StreamEvent::ToolCall {
                name, arguments, ..
            } => {
                assert_eq!(name, "file_read");
                assert_eq!(arguments["path"], "a.txt");
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
        assert!(matches!(
            &events[1],
            StreamEvent::Stop {
                reason: StopReason::ToolUse,
                usage,
            } if usage.input_tokens == 1 && usage.output_tokens == 2
        ));
    }

    #[test]
    fn tool_calls_on_non_done_then_stop_flushes_once() {
        let mut buf = Vec::new();
        let c1: ChatResponseChunk = serde_json::from_value(json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    { "function": { "name": "file_read", "arguments": { "path": "b.txt" } } }
                ],
            },
            "done": false,
        }))
        .unwrap();
        let e1 = chunk_to_events_buffered(c1, &mut buf);
        assert!(e1.is_empty(), "intermediate chunk must not emit tool calls");
        assert_eq!(buf.len(), 1);

        let c2: ChatResponseChunk = serde_json::from_value(json!({
            "message": { "role": "assistant", "content": "" },
            "done": true,
            "done_reason": "tool_calls",
            "prompt_eval_count": 3,
            "eval_count": 4,
        }))
        .unwrap();
        let e2 = chunk_to_events_buffered(c2, &mut buf);
        assert_eq!(e2.len(), 2);
        assert!(matches!(&e2[0], StreamEvent::ToolCall { name, .. } if name == "file_read"));
        assert!(matches!(&e2[1], StreamEvent::Stop { .. }));
        assert!(buf.is_empty());
    }

    #[test]
    fn done_chunk_emits_stop_event() {
        let chunk: ChatResponseChunk = serde_json::from_value(json!({
            "message": { "role": "assistant", "content": "" },
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 10,
            "eval_count": 20,
        }))
        .unwrap();
        let mut buf = Vec::new();
        let events = chunk_to_events_buffered(chunk, &mut buf);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Stop { reason, usage } => {
                assert_eq!(*reason, StopReason::EndTurn);
                assert_eq!(usage.input_tokens, 10);
                assert_eq!(usage.output_tokens, 20);
            }
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn done_with_length_maps_to_max_tokens() {
        let chunk: ChatResponseChunk = serde_json::from_value(json!({
            "done": true,
            "done_reason": "length",
        }))
        .unwrap();
        let mut buf = Vec::new();
        let events = chunk_to_events_buffered(chunk, &mut buf);
        assert!(matches!(
            &events[0],
            StreamEvent::Stop {
                reason: StopReason::MaxTokens,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn events_from_ndjson_parses_multiline_stream() {
        let body = "{\"message\":{\"role\":\"assistant\",\"content\":\"hello \"},\"done\":false}\n\
                    {\"message\":{\"role\":\"assistant\",\"content\":\"world\"},\"done\":false}\n\
                    {\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":5,\"eval_count\":3}\n";
        let bytes_stream = stream::iter(vec![Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
        let mut events: Vec<StreamEvent> = events_from_ndjson(bytes_stream)
            .try_collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(events.len(), 4, "Start + 2 deltas + Stop, got {events:?}");
        assert!(matches!(events.remove(0), StreamEvent::Start));
        assert!(matches!(&events[0], StreamEvent::TextDelta { text } if text == "hello "));
        assert!(matches!(&events[1], StreamEvent::TextDelta { text } if text == "world"));
        assert!(matches!(
            &events[2],
            StreamEvent::Stop { reason: StopReason::EndTurn, usage } if usage.input_tokens == 5 && usage.output_tokens == 3
        ));
    }
}
