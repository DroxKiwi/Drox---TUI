//! `UserAsker` bloquant : file de questions affichées une par une dans le TUI.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use drox_tools::{ToolError, UserAnswer, UserAsker, UserQuestion};
use parking_lot::Mutex;
use tokio::sync::oneshot;

/// Question en attente de réponse dans l'UI.
pub struct PendingAsk {
    pub question: UserQuestion,
    reply: oneshot::Sender<UserAnswer>,
}

impl PendingAsk {
    pub fn complete(self, answer: UserAnswer) {
        let _ = self.reply.send(answer);
    }
}

/// File partagée entre la boucle TUI et le moteur (§5.1 — permissions multiples).
#[derive(Default)]
pub struct AskCoordinator {
    queue: Mutex<VecDeque<PendingAsk>>,
}

impl AskCoordinator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retire la prochaine question pour affichage modal.
    pub fn take_next(&self) -> Option<PendingAsk> {
        self.queue.lock().pop_front()
    }

    /// Nombre de questions encore en file (hors celle affichée).
    #[must_use]
    pub fn queued_count(&self) -> usize {
        self.queue.lock().len()
    }

    /// `true` si au moins une question attend (file ou en cours côté UI).
    #[must_use]
    pub fn has_queued(&self) -> bool {
        !self.queue.lock().is_empty()
    }
}

/// Implémentation `UserAsker` pour `ToolContext`.
pub struct TuiUserAsker {
    coordinator: Arc<AskCoordinator>,
}

impl TuiUserAsker {
    #[must_use]
    pub fn new(coordinator: Arc<AskCoordinator>) -> Self {
        Self { coordinator }
    }
}

#[async_trait]
impl UserAsker for TuiUserAsker {
    async fn ask(&self, question: UserQuestion) -> Result<UserAnswer, ToolError> {
        let (tx, rx) = oneshot::channel();
        self.coordinator.queue.lock().push_back(PendingAsk {
            question,
            reply: tx,
        });
        rx.await.map_err(|_| ToolError::interactive("réponse TUI annulée"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queues_multiple_questions() {
        let coord = Arc::new(AskCoordinator::new());

        let q1 = UserQuestion {
            id: None,
            prompt: "first?".into(),
            choices: vec![],
            allow_multiple: false,
            allow_free_text: true,
        };
        let q2 = UserQuestion {
            id: None,
            prompt: "second?".into(),
            choices: vec![],
            allow_multiple: false,
            allow_free_text: true,
        };

        let h1 = tokio::spawn({
            let asker = TuiUserAsker::new(Arc::clone(&coord));
            async move { asker.ask(q1).await }
        });
        let h2 = tokio::spawn({
            let asker = TuiUserAsker::new(Arc::clone(&coord));
            async move { asker.ask(q2).await }
        });

        for _ in 0..20 {
            if coord.queued_count() == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(coord.queued_count(), 2);
        let p1 = coord.take_next().unwrap();
        p1.complete(UserAnswer {
            id: None,
            text: "a".into(),
            indices: vec![],
            skipped: false,
        });
        assert_eq!(coord.queued_count(), 1);
        let p2 = coord.take_next().unwrap();
        p2.complete(UserAnswer {
            id: None,
            text: "b".into(),
            indices: vec![],
            skipped: false,
        });
        assert_eq!(coord.queued_count(), 0);

        assert_eq!(h1.await.unwrap().unwrap().text, "a");
        assert_eq!(h2.await.unwrap().unwrap().text, "b");
    }
}
