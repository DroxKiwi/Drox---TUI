//! Données pour la barre de statut enrichie (Sprint 11).

use std::time::{Duration, Instant};

use camino::Utf8Path;

use drox_session::SessionUiStats;

use super::branch_cmd::git_branch;
use super::EngineRuntime;

/// Snapshot affiché en bas de l'écran.
#[derive(Debug, Clone)]
pub struct StatusBarSnapshot {
    pub model: String,
    pub workspace_short: String,
    pub branch: Option<String>,
    pub stats: SessionUiStats,
    pub ctx_pct: Option<u8>,
    pub session_elapsed: Duration,
}

impl Default for StatusBarSnapshot {
    fn default() -> Self {
        Self {
            model: String::new(),
            workspace_short: String::new(),
            branch: None,
            stats: SessionUiStats::default(),
            ctx_pct: None,
            session_elapsed: Duration::ZERO,
        }
    }
}

impl EngineRuntime {
    /// Met à jour le snapshot (appel async throttlé depuis la boucle TUI).
    pub async fn build_status_snapshot(
        &self,
        session_started: Instant,
    ) -> StatusBarSnapshot {
        let path = drox_session::session_ui_stats_path(
            &self.sessions_dir,
            &drox_types::SessionId::from_string(self.session_id()),
        );
        let stats = drox_session::read_session_ui_stats(&path)
            .await
            .unwrap_or_default();
        let ctx_pct = if stats.ctx > 0 && self.num_ctx() > 0 {
            Some(
                ((u64::from(stats.ctx) * 100) / self.num_ctx() as u64).min(100) as u8,
            )
        } else {
            None
        };
        StatusBarSnapshot {
            model: self.model_label(),
            workspace_short: short_workspace_label(&self.workspace),
            branch: git_branch(&self.workspace).ok(),
            stats,
            ctx_pct,
            session_elapsed: session_started.elapsed(),
        }
    }
}

fn short_workspace_label(path: &Utf8Path) -> String {
    let s = path.as_str();
    if s.len() <= 28 {
        return s.to_string();
    }
    if let Some(name) = path.file_name() {
        if name.len() <= 24 {
            return format!("…/{name}");
        }
        return format!("…/{}", &name[..21.min(name.len())]);
    }
    format!("…{}", &s[s.len().saturating_sub(24)..])
}

#[must_use]
pub fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    let rem = secs % 60;
    if mins < 60 {
        return format!("{mins}m{rem:02}s");
    }
    let hours = mins / 60;
    let mrem = mins % 60;
    format!("{hours}h{mrem:02}m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_long_workspace() {
        let p = camino::Utf8Path::new("C:/Users/someone/Documents/myproject");
        let s = short_workspace_label(p);
        assert!(s.contains("myproject"));
        assert!(s.len() <= 30);
    }

    #[test]
    fn formats_duration() {
        assert_eq!(format_elapsed(Duration::from_secs(45)), "45s");
        assert_eq!(format_elapsed(Duration::from_secs(125)), "2m05s");
    }
}
