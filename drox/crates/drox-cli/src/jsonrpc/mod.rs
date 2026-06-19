//! Serveur JSON-RPC 2.0 sur stdio (NDJSON).
//!
//! Une ligne = un message JSON-RPC. Pas d'en-têtes `Content-Length` (contrairement
//! au LSP), pour rester lisible dans un terminal et facile à scripter.
//!
//! ## Méthodes côté serveur
//!
//! | Méthode         | Description                                            |
//! |-----------------|--------------------------------------------------------|
//! | `initialize`    | Négocie protocole + retourne capabilities              |
//! | `agent.run`     | Démarre un run ; événements streamés en notifications  |
//! | `agent.cancel`  | Annule un run en cours                                 |
//! | `session.list`  | Liste les transcripts JSONL                            |
//! | `session.read`  | Charge un transcript en `[Message]`                    |
//! | `session.compact` | Tour LLM de compaction sur le transcript (sans run) |
//! | `shutdown`      | Termine proprement le serveur                          |
//!
//! ## Notifications serveur → client
//!
//! - `agent/event { runId, event }`     — événement agent streamé
//! - `agent/done  { runId, status }`    — fin de run (`completed|cancelled|error`)
//!
//! ## Requêtes serveur → client (depuis v1.1)
//!
//! - `tool/exec { runId, callId, toolName, input, workspace }` — délégation
//!   d'un tool au client (voir `executableTools` dans `clientCapabilities`).
//!   Le client répond avec `{ output, isError? }` ou un `error` JSON-RPC.
//!
//! ## Versionnage
//!
//! `PROTOCOL_VERSION` est renvoyé par `initialize`. Toute modification
//! cassante (renommage de champs, sémantique différente) **doit** incrémenter
//! ce numéro et être tracée dans `docs/GUIDE-REFONTE-DROX.md`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub mod handlers;
pub mod protocol;
pub mod remote_tool;
pub mod server;

pub use server::serve_stdio;

/// Version courante du protocole drox JSON-RPC.
pub const PROTOCOL_VERSION: &str = "1.0";

/// Identifiant de requête JSON-RPC 2.0 (`string`, `number` ou `null`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
    Null,
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Null => write!(f, "null"),
        }
    }
}

/// Requête entrante (client → serveur).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Notification entrante (client → serveur, pas de `id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Réponse entrante du client à une requête serveur→client (`tool/exec`).
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingResponse {
    // Gardé pour le matching `serde(untagged)` côté `Incoming` ; pas relu
    // ensuite (la version est validée par le simple fait d'avoir parsé).
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

/// Message reçu par le serveur.
///
/// Depuis v1.1 le serveur accepte aussi des `Response` du client en réponse à
/// ses propres requêtes (cf. `tool/exec`).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    // ATTENTION : l'ordre des variantes compte (deserialize untagged) — par
    // défaut serde ignore les champs en trop, donc une `Request` mal placée
    // matcherait `Response` (et inversement). On tente d'abord `Request`
    // (a un `method` requis), puis `Response` (a `id` + result/error, sans
    // `method`), puis `Notification` (`method` sans `id`).
    Request(Request),
    Response(IncomingResponse),
    Notification(Notification),
}

/// Réponse JSON-RPC (succès ou erreur, mais pas les deux).
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub const fn success(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    pub const fn error(id: RequestId, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

// Codes d'erreur JSON-RPC 2.0 standards
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// Codes d'erreur drox-spécifiques (plage -32000..=-32099 réservée par la spec
// JSON-RPC aux erreurs serveur applicatives).
pub const ENGINE_ERROR: i32 = -32000;
pub const CONFIG_ERROR: i32 = -32001;
pub const RUN_NOT_FOUND: i32 = -32002;
/// Le client a répondu en erreur (ou n'a pas répondu / déconnexion) à une
/// requête `tool/exec`. Le moteur traite cela comme un échec du tool.
///
/// Code documenté pour les clients qui souhaitent renvoyer une erreur typée
/// (côté serveur on encapsule plutôt dans `ToolError::Remote`).
#[allow(dead_code)]
pub const REMOTE_TOOL_ERROR: i32 = -32003;

/// Notification sortante (serveur → client).
#[derive(Debug, Clone, Serialize)]
pub struct OutgoingNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: Value,
}

impl OutgoingNotification {
    pub fn new(method: impl Into<String>, params: impl Serialize) -> serde_json::Result<Self> {
        Ok(Self {
            jsonrpc: "2.0",
            method: method.into(),
            params: serde_json::to_value(params)?,
        })
    }
}

/// Requête sortante (serveur → client). Sérialisée comme un message JSON-RPC
/// standard ; le client doit y répondre avec une `Response` portant le même
/// `id`.
#[derive(Debug, Clone, Serialize)]
pub struct OutgoingRequest {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub method: String,
    pub params: Value,
}

impl OutgoingRequest {
    pub fn new(
        id: RequestId,
        method: impl Into<String>,
        params: impl Serialize,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params: serde_json::to_value(params)?,
        })
    }
}

/// Wrapper pour décoder un `Incoming` ; renvoie une `RpcError` standard si
/// le JSON est mal formé.
pub fn parse_incoming(line: &str) -> Result<Incoming, RpcError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|e| RpcError::new(PARSE_ERROR, format!("invalid JSON: {e}")))?;
    serde_json::from_value(value).map_err(|e| {
        RpcError::new(INVALID_REQUEST, format!("invalid JSON-RPC message: {e}"))
            .with_data(json!({ "line": line }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_with_numeric_id() {
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let msg = parse_incoming(line).unwrap();
        match msg {
            Incoming::Request(r) => {
                assert_eq!(r.method, "initialize");
                assert_eq!(r.id, RequestId::Number(1));
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn parses_request_with_string_id() {
        let line = r#"{"jsonrpc":"2.0","id":"abc","method":"shutdown"}"#;
        let msg = parse_incoming(line).unwrap();
        match msg {
            Incoming::Request(r) => assert_eq!(r.id, RequestId::String("abc".into())),
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn parses_notification_when_id_missing() {
        let line = r#"{"jsonrpc":"2.0","method":"ping"}"#;
        let msg = parse_incoming(line).unwrap();
        match msg {
            Incoming::Notification(n) => assert_eq!(n.method, "ping"),
            other => panic!("expected notification, got {other:?}"),
        }
    }

    #[test]
    fn parses_response_with_result() {
        let line = r#"{"jsonrpc":"2.0","id":42,"result":{"output":"hello"}}"#;
        let msg = parse_incoming(line).unwrap();
        match msg {
            Incoming::Response(r) => {
                assert_eq!(r.id, RequestId::Number(42));
                assert!(r.error.is_none());
                assert_eq!(r.result.unwrap()["output"], json!("hello"));
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn parses_response_with_error() {
        let line = r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32603,"message":"boom"}}"#;
        let msg = parse_incoming(line).unwrap();
        match msg {
            Incoming::Response(r) => {
                let err = r.error.expect("error field");
                assert_eq!(err.code, INTERNAL_ERROR);
                assert_eq!(err.message, "boom");
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn rejects_malformed_json() {
        let err = parse_incoming("{not json}").unwrap_err();
        assert_eq!(err.code, PARSE_ERROR);
    }

    #[test]
    fn response_success_omits_error_field() {
        let resp = Response::success(RequestId::Number(7), json!({"ok": true}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""result":{"ok":true}"#));
        assert!(!s.contains("error"));
    }

    #[test]
    fn response_error_omits_result_field() {
        let resp = Response::error(
            RequestId::Number(7),
            RpcError::new(METHOD_NOT_FOUND, "no such method"),
        );
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""code":-32601"#));
        assert!(!s.contains("result"));
    }
}
