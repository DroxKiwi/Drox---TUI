//! Commandes slash asynchrones (compact, mémoire, permissions).

use anyhow::Context;
use camino::Utf8Path;
use drox_context::{microcompact_messages, MicrocompactConfig};
use drox_engine::{
    format_compact_checkpoint, summarize_run, try_live_compact, CompactionConfig, CompactionResult,
    ContextPolicy, LiveCompactReport,
};
use drox_permissions::PermissionBehavior;
use drox_session::{memory_sessions, ChatMessageRecord};
use drox_types::{Content, Message, Role};

use super::EngineRuntime;

/// Résultat d'une compaction `/compact`.
pub enum CompactOutcome {
    /// Historique réécrit sur disque + checkpoint injecté.
    Live(LiveCompactReport),
    /// Résumé seulement (historique trop court pour compaction live).
    Preview(CompactionResult),
}

impl EngineRuntime {
    /// Liste les sessions archivées `.drox/memory/sessions/`.
    pub async fn list_archived_memory(&self, limit: usize) -> anyhow::Result<Vec<String>> {
        let limit = limit.clamp(1, 50);
        let entries =
            memory_sessions::load_sessions_listing(&self.workspace, limit).await?;
        Ok(entries
            .into_iter()
            .map(|e| {
                format!(
                    "[{}] {} — {}",
                    e.date.format("%Y-%m-%d %H:%M UTC"),
                    e.slug,
                    e.objective
                )
            })
            .collect())
    }

    /// Lit le corps d'une session archivée par slug.
    pub async fn read_archived_memory(&self, slug: &str) -> anyhow::Result<String> {
        let entries =
            memory_sessions::load_sessions_listing(&self.workspace, 50).await?;
        let Some(entry) = entries.into_iter().find(|e| e.slug == slug) else {
            anyhow::bail!("aucune session mémoire avec le slug `{slug}`");
        };
        let body = memory_sessions::read_session(&entry.path).await?;
        const MAX: usize = 4000;
        Ok(if body.len() > MAX {
            format!("{}…\n\n[tronqué — {MAX} car. affichés]", &body[..MAX])
        } else {
            body
        })
    }

    /// Recherche locale dans `.drox/memory/sessions/` (équivalent TUI de `session_search`).
    pub async fn search_archived_memory(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<String>> {
        let hits = memory_sessions::search_sessions(&self.workspace, query, limit).await?;
        Ok(hits
            .into_iter()
            .map(|h| {
                format!(
                    "[score {}] {} — {}\n    {}",
                    h.score, h.slug, h.objective, h.snippet
                )
            })
            .collect())
    }

    /// Compaction LLM du transcript courant (live si possible, sinon aperçu).
    pub async fn compact_current_transcript(&self) -> anyhow::Result<CompactOutcome> {
        let mut messages = self.load_history().await;
        if messages.is_empty() {
            anyhow::bail!("transcript vide — envoyez au moins un message avant /compact");
        }

        let policy = ContextPolicy::for_model_context_window(self.num_ctx());
        let memory = self.memory.read();
        let config = memory.compaction_config.clone();
        let compaction_prompt = memory.compaction_prompt.clone();
        drop(memory);

        if let Some(report) = try_live_compact(
            self.llm().as_ref(),
            &compaction_prompt,
            &mut messages,
            &policy,
            &config,
        )
        .await
        {
            rewrite_transcript(&self.transcript_path(), &messages).await?;
            return Ok(CompactOutcome::Live(report));
        }

        let preview = summarize_run(
            self.llm().as_ref(),
            &compaction_prompt,
            &messages,
            &[],
            &CompactionConfig::default(),
        )
        .await
        .context("compaction LLM")?;
        Ok(CompactOutcome::Preview(preview))
    }

    /// Lignes pour `/permissions`.
    #[must_use]
    pub fn format_permissions_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Mode courant : {} (plan={})",
            self.permission_mode().short_title(),
            self.plan_mode()
        )];
        let grouped = self
            .permission_policy()
            .engine
            .rules()
            .group_by_source_behavior();
        if grouped.is_empty() {
            lines.push("(aucune règle chargée)".into());
            return lines;
        }
        for ((source, behavior), rules) in grouped {
            let tag = match behavior {
                PermissionBehavior::Allow => "allow",
                PermissionBehavior::Ask => "ask",
                PermissionBehavior::Deny => "deny",
            };
            lines.push(format!("— {source:?} / {tag} ({})", rules.len()));
            for r in rules.iter().take(20) {
                lines.push(format!("    {r}"));
            }
            if rules.len() > 20 {
                lines.push(format!("    … +{} règles", rules.len() - 20));
            }
        }
        lines
    }

    /// Rapport d'usage contexte (`/context`) — estimation alignée moteur.
    pub async fn format_context_lines(&self) -> Vec<String> {
        let policy = ContextPolicy::for_model_context_window(self.num_ctx());
        let budget = policy.budget();
        let effective = budget.effective_window();

        let mut messages = vec![Message::system(self.system_prompt.clone())];
        messages.extend(self.load_history().await);

        let system_tokens = policy.count_tokens(&messages[..1]);
        let transcript_tokens = policy.count_tokens(messages.get(1..).unwrap_or(&[]));

        let mut api_view = messages.clone();
        let micro = microcompact_messages(&mut api_view, &MicrocompactConfig::default());
        let total = policy.count_tokens(&api_view);
        let pct = total.saturating_mul(100) / effective.max(1);
        let warn = budget.evaluate(total);

        let mut by_role: [usize; 4] = [0, 0, 0, 0];
        let mut tool_results = 0usize;
        let mut tool_uses = 0usize;
        for msg in &api_view {
            let t = policy.count_tokens(std::slice::from_ref(msg));
            match msg.role {
                Role::System => by_role[0] += t,
                Role::User => by_role[1] += t,
                Role::Assistant => by_role[2] += t,
                Role::Tool => by_role[3] += t,
            }
            for block in &msg.content {
                match block {
                    Content::ToolResult { .. } => tool_results += 1,
                    Content::ToolUse { .. } => tool_uses += 1,
                    _ => {}
                }
            }
        }

        let mut lines = vec![
            format!("Modèle : {} · fenêtre effective ~{effective} tok", self.model_label()),
            format!("Messages : {} (transcript) + system prompt", messages.len().saturating_sub(1)),
            format!("Tokens estimés (après microcompact) : {total} / {effective} ({pct} %)"),
            format!(
                "  system prompt : {system_tokens} tok · transcript brut : {transcript_tokens} tok"
            ),
            format!(
                "  par rôle — system {0} · user {1} · assistant {2} · tool {3}",
                by_role[0], by_role[1], by_role[2], by_role[3]
            ),
            format!("  tool_use : {tool_uses} · tool_result : {tool_results}"),
        ];
        if micro.tools_cleared > 0 || micro.blocks_cleared > 0 {
            lines.push(format!(
                "  microcompact : {} résultats vidés ({} blocs)",
                micro.tools_cleared, micro.blocks_cleared
            ));
        }
        if warn.above_autocompact {
            lines.push("  ⚠ au-dessus du seuil autocompact".into());
        }
        if warn.at_blocking_limit {
            lines.push("  ⚠ limite bloquante — compaction requise avant nouveau tour".into());
        } else if warn.above_error {
            lines.push("  ⚠ contexte presque plein".into());
        } else if warn.above_warning {
            lines.push("  · marge contexte réduite".into());
        }
        lines.push(format!("  marge restante ~{} %", warn.percent_left));
        lines
    }
}

pub(crate) async fn rewrite_transcript(path: &Utf8Path, messages: &[Message]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent.as_std_path())
            .await
            .context("création répertoire transcript")?;
    }
    let mut buf = String::new();
    for msg in messages {
        let record = ChatMessageRecord::new(msg);
        buf.push_str(&serde_json::to_string(&record)?);
        buf.push('\n');
    }
    tokio::fs::write(path.as_std_path(), buf)
        .await
        .context("écriture transcript")?;
    Ok(())
}

/// Aperçu checkpoint (pour affichage fil).
#[must_use]
pub fn compact_checkpoint_preview(summary_md: &str) -> String {
    format_compact_checkpoint(summary_md)
}
