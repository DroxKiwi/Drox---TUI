//! Notes éphémères du run courant — backing store partagé entre le tool
//! `session_note` et le moteur (qui les consomme à la fin du run pour les
//! injecter dans le résumé persistant).
//!
//! Sprint M1 (mémoire unifiée). Voir aussi :
//! - [`crate::simple::SessionNoteTool`] : tool exposé au modèle.
//! - `drox-session::memory_sessions` : format markdown persistant.
//!
//! Choix de synchro : `std::sync::Mutex` (et pas `tokio::sync::Mutex`).
//! Les opérations sont triviales (un `push` ou un `drain`), brèves, et ne
//! peuvent jamais bloquer un await. Pas besoin de support cross-await.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Une note épinglée par le modèle pendant le run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionNote {
    /// Contenu libre, ≤ 500 chars (limite imposée par le tool).
    pub content: String,
    /// Horodatage UTC de la prise de note.
    pub created_at: DateTime<Utc>,
}

/// Handle thread-safe vers le stock de notes du run courant.
///
/// Cheap-to-clone : `Arc<Mutex<…>>`. Le moteur en garde une copie, le
/// `ToolContext` une autre — toutes pointent vers le même `Vec`.
#[derive(Clone, Default)]
pub struct SessionNotesHandle {
    inner: Arc<Mutex<Vec<SessionNote>>>,
}

impl SessionNotesHandle {
    /// Stock vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une note. Retourne le nombre total de notes après insertion.
    ///
    /// Le `content` est passé tel quel (la validation de taille / non-vide est
    /// faite côté tool, plus proche du message d'erreur utile au modèle).
    pub fn push(&self, content: String) -> usize {
        let note = SessionNote {
            content,
            created_at: Utc::now(),
        };
        let mut guard = self.inner.lock().expect("session notes mutex poisoned");
        guard.push(note);
        guard.len()
    }

    /// Nombre courant de notes (sans extraire les notes).
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// Indique si le stock est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Renvoie un **snapshot** des notes courantes sans les retirer.
    /// Utile pour l'inspection / les tests.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SessionNote> {
        self.inner
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Vide le stock et renvoie toutes les notes. Appelé par le moteur
    /// quand il sérialise la session à la fin du run.
    pub fn drain(&self) -> Vec<SessionNote> {
        self.inner
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default()
    }
}

impl std::fmt::Debug for SessionNotesHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionNotesHandle")
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_returns_total_count() {
        let h = SessionNotesHandle::new();
        assert_eq!(h.push("note A".into()), 1);
        assert_eq!(h.push("note B".into()), 2);
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn drain_clears_storage() {
        let h = SessionNotesHandle::new();
        h.push("a".into());
        h.push("b".into());
        let drained = h.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].content, "a");
        assert!(h.is_empty());
    }

    #[test]
    fn snapshot_does_not_clear() {
        let h = SessionNotesHandle::new();
        h.push("a".into());
        let snap = h.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(h.len(), 1, "snapshot must not consume");
    }

    #[test]
    fn handle_is_shareable_across_clones() {
        let h1 = SessionNotesHandle::new();
        let h2 = h1.clone();
        h1.push("from h1".into());
        h2.push("from h2".into());
        assert_eq!(h1.len(), 2);
        assert_eq!(h2.len(), 2);
    }
}
