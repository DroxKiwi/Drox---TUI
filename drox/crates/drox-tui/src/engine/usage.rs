//! Persistance compteurs session (`*.ui-stats.json`) — aligné JSON-RPC.

use camino::Utf8Path;
use drox_engine::AgentEvent;
use drox_session::{read_session_ui_stats, write_session_ui_stats};
use drox_types::SessionId;

use super::EngineRuntime;

impl EngineRuntime {
    /// Met à jour les stats UI session après un événement agent (async fire-and-forget).
    pub fn record_ui_stats_event(&self, ev: &AgentEvent) {
        let path = drox_session::session_ui_stats_path(
            &self.sessions_dir,
            &SessionId::from_string(self.session_id()),
        );
        let ev = ev.clone();
        tokio::spawn(async move {
            let _ = update_session_ui_stats(&path, &ev).await;
        });
    }

    /// Lignes pour `/cost`.
    pub async fn format_cost_lines(&self) -> Vec<String> {
        let path = drox_session::session_ui_stats_path(
            &self.sessions_dir,
            &SessionId::from_string(self.session_id()),
        );
        let stats = read_session_ui_stats(&path).await.unwrap_or_default();
        let ctx_lines = self.format_context_lines().await;
        let ctx_est = ctx_lines
            .iter()
            .find(|l| l.contains("Tokens estimés"))
            .cloned()
            .unwrap_or_else(|| "  (voir /context)".into());

        vec![
            "Usage session (tokens)".into(),
            format!("  session : {}", self.session_id()),
            format!("  entrée cumulée (↑) : {}", stats.total_in),
            format!("  sortie cumulée (↓) : {}", stats.total_out),
            format!("  dernier ctx (provider) : {}", stats.ctx),
            ctx_est,
            "  coût $ : N/A (LLM local — pas de facturation cloud)".into(),
            "Astuce : les cumuls se mettent à jour après chaque tour agent.".into(),
        ]
    }
}

async fn update_session_ui_stats(path: &Utf8Path, ev: &AgentEvent) -> Result<(), drox_session::SessionError> {
    let mut s = read_session_ui_stats(path).await.unwrap_or_default();
    let mut dirty = false;
    match ev {
        AgentEvent::Stop { usage, .. } => {
            if usage.input_tokens > 0 || usage.output_tokens > 0 {
                s.total_in = s.total_in.saturating_add(u64::from(usage.input_tokens));
                s.total_out = s.total_out.saturating_add(u64::from(usage.output_tokens));
                if usage.input_tokens > 0 {
                    s.ctx = usage.input_tokens;
                }
                dirty = true;
            }
        }
        AgentEvent::ContextSnip { tokens_used_after, .. } => {
            s.ctx = usize_to_u32(*tokens_used_after);
            dirty = true;
        }
        AgentEvent::ContextCompacted {
            tokens_after,
            usage,
            ..
        } => {
            s.ctx = usize_to_u32(*tokens_after);
            if let Some(u) = usage {
                if u.input_tokens > 0 || u.output_tokens > 0 {
                    s.total_in = s.total_in.saturating_add(u64::from(u.input_tokens));
                    s.total_out = s.total_out.saturating_add(u64::from(u.output_tokens));
                }
            }
            dirty = true;
        }
        _ => {}
    }
    if dirty {
        write_session_ui_stats(path, &s).await?;
    }
    Ok(())
}

fn usize_to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
