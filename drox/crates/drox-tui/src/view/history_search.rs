//! Recherche historique composer (`Ctrl+R`).

/// Mode recherche dans l'historique des prompts.
#[derive(Debug, Clone)]
pub struct HistorySearchState {
    pub query: String,
    pub saved_buffer: String,
    pub saved_history_cursor: Option<usize>,
    /// Indices dans `input_history` (ordre : du plus récent au plus ancien).
    pub match_indices: Vec<usize>,
    pub current: usize,
}

impl HistorySearchState {
    #[must_use]
    pub fn counter_label(&self) -> String {
        if self.query.is_empty() {
            return "filtre historique".into();
        }
        if self.match_indices.is_empty() {
            return "aucun match".into();
        }
        format!("{}/{}", self.current + 1, self.match_indices.len())
    }

    #[must_use]
    pub fn current_history_index(&self) -> Option<usize> {
        self.match_indices.get(self.current).copied()
    }

    pub fn next_match(&mut self) {
        if self.match_indices.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.match_indices.len();
    }
}

/// Recalcule les entrées d'historique correspondant à `query` (récent → ancien).
#[must_use]
pub fn find_history_match_indices(history: &[String], query: &str) -> Vec<usize> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let q_lower = q.to_lowercase();
    history
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(idx, entry)| {
            if entry.to_lowercase().contains(&q_lower) {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_matches_newest_first() {
        let h = vec!["alpha".into(), "beta test".into(), "gamma".into()];
        let hits = find_history_match_indices(&h, "test");
        assert_eq!(hits, vec![1]);
    }
}
