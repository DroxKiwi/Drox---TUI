//! Boucle stdio + dispatcher du serveur JSON-RPC.
//!
//! Le serveur est sans état partagé global : tout vit dans une [`Server`]
//! locale à l'appelant, ce qui rend les tests faciles (on peut injecter un
//! canal mpsc à la place de `stdout` réel).

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use super::handlers::{self, RunOutcome};
use super::{
    INTERNAL_ERROR, Incoming, IncomingResponse, METHOD_NOT_FOUND, OutgoingNotification,
    OutgoingRequest, Request, RequestId, Response, RpcError, parse_incoming,
};

/// Handle vers une tâche `agent.run` active.
struct RunHandle {
    join: JoinHandle<RunOutcome>,
}

/// Slot d'une requête sortante en attente de réponse client.
type PendingSlot = oneshot::Sender<Result<Value, RpcError>>;

/// État partagé du serveur, cheap-to-clone via `Arc`.
#[derive(Clone)]
pub struct Server {
    out: mpsc::Sender<String>,
    runs: Arc<Mutex<HashMap<String, RunHandle>>>,
    next_run_id: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    pending_outbound: Arc<Mutex<HashMap<i64, PendingSlot>>>,
    next_outbound_id: Arc<AtomicI64>,
    /// Tools que le client a déclaré pouvoir exécuter (via
    /// `clientCapabilities.executableTools` à `initialize`).
    executable_tools: Arc<Mutex<HashSet<String>>>,
    /// Vrai si le client a déclaré supporter la requête `user/ask` (carte
    /// Questions bloquantes — §2.13). Sélectionne `RpcUserAsker` (si vrai)
    /// ou `RefuseAsker` (si faux/absent) au démarrage d'un `agent.run`.
    interactive_ask: Arc<AtomicBool>,
}

impl Server {
    pub fn new(out: mpsc::Sender<String>) -> Self {
        Self {
            out,
            runs: Arc::new(Mutex::new(HashMap::new())),
            next_run_id: Arc::new(AtomicU64::new(1)),
            shutdown: Arc::new(AtomicBool::new(false)),
            pending_outbound: Arc::new(Mutex::new(HashMap::new())),
            next_outbound_id: Arc::new(AtomicI64::new(1)),
            executable_tools: Arc::new(Mutex::new(HashSet::new())),
            interactive_ask: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Déclare les tools que le client peut exécuter à distance. Remplace la
    /// liste précédente.
    pub fn set_executable_tools(&self, names: impl IntoIterator<Item = String>) {
        let mut set = self.executable_tools.lock();
        set.clear();
        for name in names {
            set.insert(name);
        }
    }

    /// Mémorise la capacité `interactiveAsk` annoncée par le client à
    /// `initialize`. Voir [`Self::supports_interactive_ask`].
    pub fn set_interactive_ask(&self, value: bool) {
        self.interactive_ask.store(value, Ordering::SeqCst);
    }

    /// Vrai si le client a déclaré supporter `user/ask` (carte Questions
    /// bloquantes — §2.13 du backlog).
    pub fn supports_interactive_ask(&self) -> bool {
        self.interactive_ask.load(Ordering::SeqCst)
    }

    /// Liste actuelle des tools exécutables côté client.
    pub fn executable_tools(&self) -> Vec<String> {
        self.executable_tools.lock().iter().cloned().collect()
    }

    /// Indique si un tool donné est délégué au client. Conservé pour les
    /// tests et l'introspection ; le wiring effectif passe par
    /// `Self::executable_tools` lors de la construction de la registry.
    #[allow(dead_code)]
    pub fn is_executable_remotely(&self, name: &str) -> bool {
        self.executable_tools.lock().contains(name)
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    pub fn allocate_run_id(&self) -> String {
        let n = self.next_run_id.fetch_add(1, Ordering::SeqCst);
        format!("run_{n}")
    }

    pub fn register_run(&self, run_id: String, join: JoinHandle<RunOutcome>) {
        self.runs.lock().insert(run_id, RunHandle { join });
    }

    /// Tente d'annuler un run. Renvoie `true` si le run était présent.
    pub fn cancel_run(&self, run_id: &str) -> bool {
        let maybe = self.runs.lock().remove(run_id);
        maybe.is_some_and(|handle| {
            handle.join.abort();
            true
        })
    }

    pub fn forget_run(&self, run_id: &str) {
        self.runs.lock().remove(run_id);
    }

    /// Attend la fin de tous les runs encore actifs. Appelé après EOF stdin
    /// ou `shutdown` pour ne pas perdre les notifications de fin de stream.
    pub async fn drain_runs(&self) {
        let handles: Vec<_> = self.runs.lock().drain().map(|(_, h)| h.join).collect();
        for h in handles {
            let _ = h.await;
        }
    }

    /// Envoie une notification au client (best-effort). Si le canal est fermé,
    /// log un warn et ignore.
    pub async fn notify<P: Serialize>(&self, method: &str, params: P) {
        match OutgoingNotification::new(method, params) {
            Ok(n) => {
                if let Err(e) = self.send_value(&n).await {
                    warn!(error = %e, "failed to send notification");
                }
            }
            Err(e) => warn!(error = %e, "failed to serialize notification"),
        }
    }

    /// Envoie une réponse de requête.
    pub async fn respond(&self, response: Response) {
        if let Err(e) = self.send_value(&response).await {
            warn!(error = %e, "failed to send response");
        }
    }

    /// Envoie une **requête serveur→client** et attend la `Response` du
    /// client. Renvoie l'erreur RPC du client telle quelle, ou un
    /// `INTERNAL_ERROR` si le canal se ferme avant réponse.
    ///
    /// L'`id` JSON-RPC est généré ici (numérique, croissant). Ne pas confondre
    /// avec le `callId` interne porté dans les `params` de `tool/exec`.
    pub async fn send_request<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Value, RpcError> {
        let id = self.next_outbound_id.fetch_add(1, Ordering::SeqCst);
        let req = OutgoingRequest::new(RequestId::Number(id), method, params)
            .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("serialize params: {e}")))?;

        let (tx, rx) = oneshot::channel();
        self.pending_outbound.lock().insert(id, tx);

        if let Err(e) = self.send_value(&req).await {
            self.pending_outbound.lock().remove(&id);
            return Err(RpcError::new(
                INTERNAL_ERROR,
                format!("failed to send request: {e}"),
            ));
        }

        rx.await.unwrap_or_else(|_| {
            // Le `oneshot::Sender` a été drop sans réponse — soit le client a
            // coupé, soit la map a été vidée. On retourne une erreur dédiée.
            self.pending_outbound.lock().remove(&id);
            Err(RpcError::new(
                INTERNAL_ERROR,
                "client disconnected before reply",
            ))
        })
    }

    /// Si l'`id` correspond à une requête sortante en attente, livre la
    /// réponse via le `oneshot` correspondant.
    fn dispatch_incoming_response(&self, resp: IncomingResponse) {
        let id = match resp.id {
            RequestId::Number(n) => n,
            other => {
                debug!(?other, "incoming response with non-numeric id ignored");
                return;
            }
        };
        let Some(tx) = self.pending_outbound.lock().remove(&id) else {
            debug!(id, "incoming response with no matching pending request");
            return;
        };
        let value = match (resp.result, resp.error) {
            (Some(v), None) => Ok(v),
            (_, Some(err)) => Err(err),
            (None, None) => Err(RpcError::new(
                INTERNAL_ERROR,
                "response without result nor error",
            )),
        };
        let _ = tx.send(value);
    }

    async fn send_value<T: Serialize + Sync>(&self, value: &T) -> Result<(), String> {
        let s = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.out.send(s).await.map_err(|e| e.to_string())
    }

    /// Traite une ligne JSON-RPC. Spawne éventuellement une tâche pour
    /// `agent.run`, mais ne **bloque jamais** sur l'agent.
    pub async fn handle_line(&self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        match parse_incoming(trimmed) {
            Ok(Incoming::Request(req)) => self.handle_request(req).await,
            Ok(Incoming::Response(resp)) => self.dispatch_incoming_response(resp),
            Ok(Incoming::Notification(n)) => {
                debug!(method = %n.method, "ignored client notification");
            }
            Err(err) => {
                // Pas d'`id` disponible (parse error), on renvoie `null` comme
                // le permet la spec JSON-RPC 2.0.
                self.respond(Response::error(RequestId::Null, err)).await;
            }
        }
    }

    async fn handle_request(&self, req: Request) {
        let id = req.id.clone();
        match req.method.as_str() {
            "initialize" => match handlers::initialize(self, req.params).await {
                Ok(v) => self.respond(Response::success(id, v)).await,
                Err(e) => self.respond(Response::error(id, e)).await,
            },
            "shutdown" => {
                self.request_shutdown();
                self.respond(Response::success(id, serde_json::Value::Null))
                    .await;
            }
            "session.list" => match handlers::session_list(req.params).await {
                Ok(v) => self.respond(Response::success(id, v)).await,
                Err(e) => self.respond(Response::error(id, e)).await,
            },
            "session.read" => match handlers::session_read(req.params).await {
                Ok(v) => self.respond(Response::success(id, v)).await,
                Err(e) => self.respond(Response::error(id, e)).await,
            },
            "session.compact" => match handlers::session_compact(req.params).await {
                Ok(v) => self.respond(Response::success(id, v)).await,
                Err(e) => self.respond(Response::error(id, e)).await,
            },
            "agent.run" => match handlers::agent_run(self.clone(), req.params).await {
                Ok(v) => self.respond(Response::success(id, v)).await,
                Err(e) => self.respond(Response::error(id, e)).await,
            },
            "agent.cancel" => match handlers::agent_cancel(self, req.params) {
                Ok(v) => self.respond(Response::success(id, v)).await,
                Err(e) => self.respond(Response::error(id, e)).await,
            },
            other => {
                self.respond(Response::error(
                    id,
                    RpcError::new(METHOD_NOT_FOUND, format!("unknown method: {other}")),
                ))
                .await;
            }
        }
    }
}

/// Point d'entrée du mode `--serve` du binaire `drox`.
///
/// Lit NDJSON sur stdin, écrit NDJSON sur stdout, jusqu'à EOF ou `shutdown`.
pub async fn serve_stdio() -> anyhow::Result<()> {
    let (out_tx, mut out_rx) = mpsc::channel::<String>(64);

    let writer_task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(line) = out_rx.recv().await {
            if stdout.write_all(line.as_bytes()).await.is_err()
                || stdout.write_all(b"\n").await.is_err()
            {
                break;
            }
            let _ = stdout.flush().await;
        }
    });

    let server = Server::new(out_tx);

    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut buf = String::new();
    loop {
        buf.clear();
        match stdin.read_line(&mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                server.handle_line(&buf).await;
                if server.is_shutdown() {
                    break;
                }
            }
            Err(e) => {
                warn!(error = %e, "stdin read error, terminating");
                break;
            }
        }
    }

    server.drain_runs().await;
    drop(server);
    let _ = writer_task.await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn test_server() -> (Server, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel::<String>(32);
        (Server::new(tx), rx)
    }

    async fn collect_one(rx: &mut mpsc::Receiver<String>) -> Value {
        let s = rx.recv().await.expect("expected one message");
        serde_json::from_str(&s).unwrap()
    }

    #[tokio::test]
    async fn initialize_returns_capabilities() {
        let (server, mut rx) = test_server();
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        server.handle_line(line).await;
        let resp = collect_one(&mut rx).await;
        assert_eq!(resp["id"], json!(1));
        assert_eq!(resp["result"]["serverName"], json!("drox"));
        assert_eq!(resp["result"]["protocolVersion"], json!("1.0"));
        assert_eq!(
            resp["result"]["capabilities"]["runStreamingEvents"],
            json!(true)
        );
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let (server, mut rx) = test_server();
        let line = r#"{"jsonrpc":"2.0","id":"x","method":"no.such.method"}"#;
        server.handle_line(line).await;
        let resp = collect_one(&mut rx).await;
        assert_eq!(resp["id"], json!("x"));
        assert_eq!(resp["error"]["code"], json!(METHOD_NOT_FOUND));
    }

    #[tokio::test]
    async fn invalid_json_returns_parse_error() {
        let (server, mut rx) = test_server();
        server.handle_line("{not json").await;
        let resp = collect_one(&mut rx).await;
        assert_eq!(resp["id"], Value::Null);
        assert_eq!(resp["error"]["code"], json!(super::super::PARSE_ERROR));
    }

    #[tokio::test]
    async fn shutdown_request_sets_flag_and_responds_null() {
        let (server, mut rx) = test_server();
        assert!(!server.is_shutdown());
        server
            .handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#)
            .await;
        let resp = collect_one(&mut rx).await;
        assert_eq!(resp["result"], Value::Null);
        assert!(server.is_shutdown());
    }

    #[tokio::test]
    async fn cancel_unknown_run_returns_run_not_found() {
        let (server, mut rx) = test_server();
        server
            .handle_line(
                r#"{"jsonrpc":"2.0","id":1,"method":"agent.cancel","params":{"runId":"run_999"}}"#,
            )
            .await;
        let resp = collect_one(&mut rx).await;
        assert_eq!(resp["error"]["code"], json!(super::super::RUN_NOT_FOUND));
    }

    #[tokio::test]
    async fn allocate_run_id_is_monotonic() {
        let (server, _rx) = test_server();
        let a = server.allocate_run_id();
        let b = server.allocate_run_id();
        assert_eq!(a, "run_1");
        assert_eq!(b, "run_2");
    }

    #[tokio::test]
    async fn send_request_resolves_with_client_result() {
        let (server, mut rx) = test_server();
        let s = server.clone();
        let pending =
            tokio::spawn(async move { s.send_request("tool/exec", json!({"x": 1})).await });

        // Lire la requête envoyée et récupérer son id.
        let line = rx.recv().await.expect("server should send request");
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["method"], json!("tool/exec"));
        assert_eq!(parsed["params"], json!({"x": 1}));
        let id = parsed["id"].as_i64().expect("numeric id");

        // Simuler la réponse du client.
        let resp = format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"output":"ok"}}}}"#);
        server.handle_line(&resp).await;

        let value = pending.await.unwrap().expect("ok");
        assert_eq!(value["output"], json!("ok"));
    }

    #[tokio::test]
    async fn send_request_propagates_client_error() {
        let (server, mut rx) = test_server();
        let s = server.clone();
        let pending = tokio::spawn(async move { s.send_request("tool/exec", json!({})).await });

        let line = rx.recv().await.unwrap();
        let parsed: Value = serde_json::from_str(&line).unwrap();
        let id = parsed["id"].as_i64().unwrap();
        let resp =
            format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-32603,"message":"boom"}}}}"#);
        server.handle_line(&resp).await;

        let err = pending.await.unwrap().unwrap_err();
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(err.message, "boom");
    }

    #[tokio::test]
    async fn send_request_returns_error_when_channel_closes() {
        let (server, _rx) = test_server();
        // On droppe le receiver (déjà fait via le `_rx` qui sort de scope à la
        // fin du test) — pour simuler la déconnexion, on retire le pending
        // sans répondre.
        let pending = {
            let s = server.clone();
            tokio::spawn(async move { s.send_request("tool/exec", json!({})).await })
        };
        // Donne le temps à la tâche d'enregistrer son pending.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        // Vide la map : émule un client qui n'a jamais répondu et a coupé.
        server.pending_outbound.lock().clear();
        let err = pending.await.unwrap().unwrap_err();
        assert_eq!(err.code, INTERNAL_ERROR);
        assert!(err.message.contains("client disconnected"));
    }

    #[tokio::test]
    async fn executable_tools_register_and_query() {
        let (server, _rx) = test_server();
        assert!(!server.is_executable_remotely("file_write"));
        server.set_executable_tools(["file_write".to_string(), "file_edit".to_string()]);
        assert!(server.is_executable_remotely("file_write"));
        assert!(!server.is_executable_remotely("bash"));
        let mut listed = server.executable_tools();
        listed.sort();
        assert_eq!(
            listed,
            vec!["file_edit".to_string(), "file_write".to_string()]
        );
    }
}
