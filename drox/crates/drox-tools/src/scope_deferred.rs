//! Findings « hors scope » reportés via `scope_defer` pendant un run (§2.25).

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

/// Entrée de parking hors scope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeDeferredItem {
    pub finding: String,
    pub reason: String,
}

/// Stock thread-safe des éléments reportés pendant le run.
#[derive(Clone, Default)]
pub struct ScopeDeferredHandle {
    inner: Arc<Mutex<Vec<ScopeDeferredItem>>>,
}

impl ScopeDeferredHandle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, finding: String, reason: String) -> usize {
        let item = ScopeDeferredItem { finding, reason };
        let mut guard = self.inner.lock().expect("scope_deferred mutex poisoned");
        guard.push(item);
        guard.len()
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<ScopeDeferredItem> {
        self.inner
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }
}
