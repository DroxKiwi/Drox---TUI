//! Binaire `drox`.
//!
//! Parse les arguments CLI, configure le runtime, instancie l'agent
//! (`drox-engine`) et streame ses événements sur stdout.
//!
//! À terme, expose aussi un mode JSON-RPC sur stdio pour les clients UI
//! (extension VS Code) au sprint 1.11.

mod asker;
mod jsonrpc;

use drox_cli::{env_file, language, prompts};

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use anyhow::Context;
use camino::Utf8PathBuf;
use clap::Parser;
use drox_engine::{
    Agent, AgentConfig, AgentEvent, ContextPolicy, JsonlTranscriptSink, LayeredConfig,
    PermissionEngine, PermissionMode, PermissionPolicy, SessionError, TranscriptSessionConfig,
    default_sessions_dir, default_tool_registry, list_sessions, load_memdir, memdir_system_prefix,
    read_transcript, transcript_path,
};
use drox_llm::{ChatOptions, LlmConfig, OllamaClient};
use drox_permissions::{
    DetectUnreachableOptions, PathMatchContext, PermissionBehavior, Rule, RuleSet, RuleSource,
    detect_unreachable_rules, format_rule, parse_rule,
};
use drox_tools::ToolContext;
use drox_types::SessionId;
use futures::StreamExt;
use tracing_subscriber::EnvFilter;

use crate::asker::StdinUserAsker;

#[derive(Debug, Parser)]
#[command(
    name = "drox",
    version,
    about = "Moteur agent Drox — autonome, branché sur un LLM (Ollama par défaut)"
)]
#[allow(clippy::struct_excessive_bools)] // struct d'arguments clap : nombreux flags booléens attendus
struct Cli {
    /// URL du serveur LLM (par défaut <http://localhost:11434> pour Ollama).
    #[arg(long, env = "DROX_SERVER", default_value = "http://localhost:11434")]
    server: String,

    /// Modèle à utiliser (e.g. llama3.2, qwen2.5-coder:32b).
    #[arg(long, env = "DROX_MODEL", default_value = "llama3.2")]
    model: String,

    /// Racine du workspace exposée aux tools (par défaut : `cwd`).
    #[arg(long, env = "DROX_WORKSPACE")]
    workspace: Option<Utf8PathBuf>,

    /// Si défini, les tools `file_write`/`file_edit` appliquent réellement
    /// les écritures. Sinon, ils retournent une proposition JSON.
    #[arg(long)]
    apply: bool,

    /// Active le mode plan : aucun tool d'écriture ne peut s'exécuter.
    /// L'agent doit terminer sa réflexion par `exit_plan_mode` pour valider.
    #[arg(long)]
    plan: bool,

    /// Mode de permission : `default`, `plan`, `acceptEdits`, `bypassPermissions`.
    /// Si non précisé, lu depuis `settings.json` puis `default`.
    /// `--plan` est équivalent à `--mode plan`.
    #[arg(long)]
    mode: Option<String>,

    /// Règle à autoriser inconditionnellement, ex. `Bash(npm:*)` (peut être
    /// répété). Source `cliArg`, prioritaire sur les fichiers de settings.
    #[arg(long = "allow")]
    allow: Vec<String>,

    /// Règle à demander avant exécution, ex. `Bash(git push:*)` (répétable).
    #[arg(long = "ask")]
    ask: Vec<String>,

    /// Règle à refuser inconditionnellement, ex. `Bash(rm -rf *)` (répétable).
    #[arg(long = "deny")]
    deny: Vec<String>,

    /// Désactive le chargement de `~/.drox/settings.json` et
    /// `<workspace>/.drox/settings.json`. Utile pour les tests.
    #[arg(long)]
    no_settings: bool,

    /// Nombre max de tours LLM ↔ tools dans une exécution (default: 12).
    #[arg(long, default_value_t = 12)]
    max_iterations: usize,

    /// System prompt optionnel injecté en tête de conversation.
    #[arg(long)]
    system: Option<String>,

    /// Verbosité des logs (`-v`, `-vv`, `-vvv`).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Prompt à envoyer au modèle. Si omis, affiche juste la config et sort.
    #[arg(short = 'p', long)]
    prompt: Option<String>,

    /// Température de sampling (0.0 - 2.0).
    #[arg(long)]
    temperature: Option<f32>,

    /// Nombre maximum de tokens à générer par tour.
    #[arg(long)]
    max_tokens: Option<u32>,

    /// Liste les sessions (`ses_*.jsonl`) puis quitte.
    #[arg(long)]
    list_sessions: bool,

    /// Répertoire des transcripts (défaut : `~/.drox/sessions`).
    #[arg(long)]
    session_dir: Option<Utf8PathBuf>,

    /// Identifiant `ses_…` : charge `<dir>/<id>.jsonl` s'il existe et
    /// enregistre la suite de la conversation dans le même fichier.
    #[arg(long)]
    session: Option<String>,

    /// Démarre le serveur JSON-RPC v1 sur stdio (NDJSON, une ligne par message).
    /// Tous les autres flags sont ignorés (la configuration se fait par
    /// requête `agent.run`). Voir `docs/PROTOCOLE-JSONRPC.md`.
    #[arg(long)]
    serve: bool,

    /// API key envoyée comme header HTTP `x-api-key` au serveur LLM. Utile
    /// quand Ollama est exposé derrière un reverse-proxy / API gateway.
    /// Lue depuis `DROX_API_KEY` à défaut.
    #[arg(long, env = "DROX_API_KEY")]
    api_key: Option<String>,

    /// Header HTTP supplémentaire au format `Name: Value`. Répétable.
    /// Ex. `--header "Authorization: Bearer xxx"`.
    #[arg(long = "header")]
    headers: Vec<String>,
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "drox={level},drox_llm={level},drox_engine={level},drox_tools={level}"
        ))
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Construit la `PermissionPolicy` à partir des arguments CLI + des fichiers
/// `~/.drox/settings.json`, `<workspace>/.drox/settings.json` et
/// `settings.local.json` (sauf si `--no-settings`).
fn build_permission_policy(
    cli: &Cli,
    workspace: &Utf8PathBuf,
) -> anyhow::Result<(PermissionMode, PermissionPolicy)> {
    let (user_path, project_path, local_path) = if cli.no_settings {
        (None, None, None)
    } else {
        let user = dirs::home_dir().map(|h| h.join(".drox").join("settings.json"));
        let drox_dir = workspace.as_std_path().join(".drox");
        let project = Some(drox_dir.join("settings.json"));
        let local = Some(drox_dir.join("settings.local.json"));
        (user, project, local)
    };

    let layered = LayeredConfig::load(
        user_path.as_deref(),
        project_path.as_deref(),
        local_path.as_deref(),
    )
    .context("failed to load drox settings")?;

    let mut rules: RuleSet = layered.build_rule_set();
    push_cli_rules(&mut rules, &cli.allow, PermissionBehavior::Allow);
    push_cli_rules(&mut rules, &cli.ask, PermissionBehavior::Ask);
    push_cli_rules(&mut rules, &cli.deny, PermissionBehavior::Deny);

    let mode = if cli.plan {
        PermissionMode::Plan
    } else if let Some(raw) = cli.mode.as_deref() {
        PermissionMode::from_str_lossy(raw)
    } else if let Some(m) = layered.effective_mode() {
        m
    } else {
        PermissionMode::Default
    };

    for unreachable in detect_unreachable_rules(&rules, &DetectUnreachableOptions::default()) {
        tracing::warn!(
            rule = %format_rule(&unreachable.rule.value),
            reason = %unreachable.reason,
            fix = %unreachable.fix,
            "règle de permission inatteignable (masquée)"
        );
    }

    let home = dirs::home_dir().unwrap_or_else(|| workspace.as_std_path().to_path_buf());
    let engine = Arc::new(
        PermissionEngine::with_rules(rules).with_path_context(PathMatchContext::new(
            workspace.as_std_path(),
            home,
        )),
    );
    Ok((mode, PermissionPolicy::new(engine, mode)))
}

fn push_cli_rules(set: &mut RuleSet, raw: &[String], behavior: PermissionBehavior) {
    for s in raw {
        set.push(Rule {
            value: parse_rule(s),
            behavior,
            source: RuleSource::CliArg,
        });
    }
}

/// Découpe un header HTTP au format `Name: Value` ou `Name:Value`. Tolère
/// les espaces. Renvoie une erreur si le `:` est absent ou si le nom est
/// vide.
fn parse_header_arg(raw: &str) -> anyhow::Result<(String, String)> {
    let (name, value) = raw
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid --header `{raw}` (expected `Name: Value`)"))?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        anyhow::bail!("invalid --header `{raw}` (empty header name)");
    }
    Ok((name.to_string(), value.to_string()))
}

/// Fusionne `--api-key` (raccourci) et `--header K:V` (répétables) en une
/// map normalisée (noms lowercased pour éviter les doublons).
fn collect_headers(
    api_key: Option<&str>,
    raw_headers: &[String],
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    if let Some(k) = api_key {
        out.insert("x-api-key".into(), k.to_string());
    }
    for raw in raw_headers {
        let (n, v) = parse_header_arg(raw)?;
        out.insert(n.to_lowercase(), v);
    }
    Ok(out)
}

fn merge_optional_system(cli: Option<String>, mem: Option<String>) -> Option<String> {
    match (cli, mem) {
        (None, None) => None,
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (Some(a), Some(b)) => Some(format!("{a}\n\n{b}")),
    }
}

/// Lit une variable d'environnement et tente de la parser en `i64`. Aligné
/// sur l'helper `env_i64` du module JSON-RPC : utilisé pour
/// `DROX_NUM_PREDICT` / `DROX_NUM_CTX` / `DROX_TOP_K` / `DROX_SEED`.
fn parse_env_i64(key: &str) -> Option<i64> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<i64>().ok()
        }
    })
}

/// Pendant `f32`. Utilisé pour `DROX_TOP_P` / `DROX_REPEAT_PENALTY`.
fn parse_env_f32(key: &str) -> Option<f32> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<f32>().ok()
        }
    })
}

fn resolve_workspace(arg: Option<Utf8PathBuf>) -> anyhow::Result<Utf8PathBuf> {
    let raw = match arg {
        Some(p) => p,
        None => {
            Utf8PathBuf::try_from(std::env::current_dir()?).context("cwd is not valid UTF-8")?
        }
    };
    let canonical = std::fs::canonicalize(raw.as_std_path())
        .with_context(|| format!("workspace not found: {raw}"))?;
    Utf8PathBuf::try_from(canonical).context("workspace path is not valid UTF-8")
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> anyhow::Result<()> {
    // Auto-load des fichiers `.env` : workspace (cwd) puis user (`~/.drox/env`).
    // Les vars d'env déjà définies par le shell sont prioritaires.
    let cwd = std::env::current_dir().ok();
    env_file::load_default(cwd.as_deref());

    let cli = Cli::parse();
    init_tracing(cli.verbose);

    tracing::info!(server = %cli.server, model = %cli.model, "drox démarre");

    if cli.serve {
        tracing::info!("drox : mode JSON-RPC stdio (sprint 1.11)");
        return jsonrpc::serve_stdio().await;
    }

    if cli.list_sessions {
        let dir = cli
            .session_dir
            .clone()
            .unwrap_or(default_sessions_dir().context("résolution répertoire sessions")?);
        let entries = list_sessions(&dir).await.context("liste des sessions")?;
        println!("{:<42} {:>12} {:>10}", "SESSION_ID", "MODIFIED_S", "BYTES");
        for e in entries {
            println!("{:<42} {:>12} {:>10}", e.id, e.modified_secs, e.size_bytes);
        }
        return Ok(());
    }

    let Some(prompt) = cli.prompt.as_deref() else {
        println!(
            "drox v{} — moteur agent (sprint 1.11)",
            env!("CARGO_PKG_VERSION")
        );
        println!("  server   : {}", cli.server);
        println!("  model    : {}", cli.model);
        println!("  apply    : {}", cli.apply);
        println!("  plan     : {}", cli.plan);
        println!("  mode     : {}", cli.mode.as_deref().unwrap_or("(auto)"));
        println!(
            "  rules    : allow={}, ask={}, deny={}",
            cli.allow.len(),
            cli.ask.len(),
            cli.deny.len()
        );
        println!("  max_iter : {}", cli.max_iterations);
        println!();
        println!("  --list-sessions   Lister les transcripts ~/.drox/sessions");
        println!("  --session SES_ID  Reprendre / persister une session JSONL");
        println!("  --serve           Démarrer le serveur JSON-RPC v1 sur stdio");
        println!();
        println!("Utiliser `--prompt \"...\"` pour lancer une session agent.");
        return Ok(());
    };

    let headers = collect_headers(cli.api_key.as_deref(), &cli.headers)
        .context("invalid HTTP header argument")?;
    let mut config = LlmConfig::try_from_str(&cli.server, &cli.model)
        .with_context(|| format!("invalid server URL: {}", cli.server))?;
    for (k, v) in &headers {
        config = config.with_header(k, v);
    }
    if let Some(n) = parse_env_i64("DROX_NUM_PREDICT") {
        config = config.with_num_predict(n);
    }
    if let Some(n) = parse_env_i64("DROX_NUM_CTX") {
        config = config.with_num_ctx(n);
    }
    if let Some(v) = parse_env_f32("DROX_TOP_P") {
        config = config.with_top_p(Some(v));
    }
    if let Some(n) = parse_env_i64("DROX_TOP_K") {
        config = config.with_top_k(Some(n));
    }
    if let Some(v) = parse_env_f32("DROX_REPEAT_PENALTY") {
        config = config.with_repeat_penalty(Some(v));
    }
    if let Some(n) = parse_env_i64("DROX_SEED") {
        config = config.with_seed(Some(n));
    }
    if let Some(v) = parse_env_f32("DROX_MIN_P") {
        config = config.with_min_p(Some(v));
    }
    if let Some(v) = parse_env_f32("DROX_PRESENCE_PENALTY") {
        config = config.with_presence_penalty(Some(v));
    }
    if let Some(v) = parse_env_f32("DROX_FREQUENCY_PENALTY") {
        config = config.with_frequency_penalty(Some(v));
    }
    if let Ok(s) = std::env::var("DROX_KEEP_ALIVE") {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            config = config.with_keep_alive(Some(trimmed.to_owned()));
        }
    }
    tracing::info!(
        headers = headers.len(),
        api_key_set = cli.api_key.is_some(),
        num_predict = config.num_predict,
        num_ctx = config.num_ctx,
        top_p = ?config.top_p,
        top_k = ?config.top_k,
        repeat_penalty = ?config.repeat_penalty,
        seed = ?config.seed,
        min_p = ?config.min_p,
        presence_penalty = ?config.presence_penalty,
        frequency_penalty = ?config.frequency_penalty,
        keep_alive = ?config.keep_alive,
        "LLM config prête"
    );
    let num_ctx = config.num_ctx.max(2048) as usize;
    let llm = Arc::new(OllamaClient::new(config).context("failed to build Ollama client")?);
    let registry = Arc::new(default_tool_registry());
    let workspace = resolve_workspace(cli.workspace.clone())?;
    let workspace_fingerprint = workspace.as_str().to_string();
    tracing::info!(%workspace, apply = cli.apply, plan = cli.plan, "workspace résolu");

    let mem = load_memdir(workspace.as_path())
        .await
        .context("lecture MEMORY.md / DROX.md")?;
    let base_system = merge_optional_system(cli.system.clone(), memdir_system_prefix(&mem));
    let mut base_system = prompts::prepend_core_system_prompt(base_system);
    // Sprint M1 — listing des sessions archivées injecté en début de prompt.
    // Échec silencieux : un dossier .drox/memory/sessions/ illisible ne
    // doit pas casser le démarrage CLI.
    let memory_listing = drox_engine::load_sessions_listing(
        workspace.as_path(),
        drox_engine::DEFAULT_LISTING_LIMIT,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "memory: failed to load sessions listing");
        Vec::new()
    });
    if let Some(block) = drox_engine::format_sessions_listing_for_prompt(&memory_listing) {
        base_system.push_str("\n\n");
        base_system.push_str(&block);
    }
    let skills_listing = drox_engine::load_skills_catalog(workspace.as_path())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "skills: failed to load catalog");
            Vec::new()
        });
    if let Some(block) = drox_engine::format_skills_listing_for_prompt(&skills_listing) {
        base_system.push_str("\n\n");
        base_system.push_str(&block);
    }
    let base_system = Some(base_system);
    let lang = language::from_env();
    if let Some(ref l) = lang {
        tracing::info!(language = %l.display, "langue principale appliquée");
    }
    let mut system_merged = language::merge_into_system(base_system, lang.as_ref());

    let sessions_dir = cli
        .session_dir
        .clone()
        .unwrap_or(default_sessions_dir().context("résolution répertoire sessions")?);

    let (history, transcript) = if let Some(ref sid) = cli.session {
        if !sid.starts_with("ses_") {
            anyhow::bail!("--session doit être un identifiant ses_…");
        }
        let id = SessionId::from_string(sid.clone());
        let path = transcript_path(&sessions_dir, &id);
        let history = match read_transcript(&path).await {
            Ok(h) => h,
            Err(SessionError::NotFound(_)) => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        let append_from = history.len() + usize::from(system_merged.is_some());
        let sink = JsonlTranscriptSink::arc(path);
        (
            history,
            Some(TranscriptSessionConfig {
                sink,
                append_from_message_index: append_from,
            }),
        )
    } else {
        (Vec::new(), None)
    };

    let (mode, policy) = build_permission_policy(&cli, &workspace)?;
    tracing::info!(mode = mode.short_title(), "mode permission");

    let drox_ignore = drox_engine::DroxIgnoreMatcher::load_or_create(workspace.clone()).await?;
    let workspace_map = drox_engine::WorkspaceMapStore::load_or_create(
        workspace.clone(),
        workspace_fingerprint.clone(),
        Some(drox_ignore.clone()),
    )
    .await?;
    let mut base_for_map = system_merged.clone().unwrap_or_default();
    base_for_map.push_str("\n\n");
    base_for_map.push_str(&drox_ignore.format_for_prompt());
    if let Some(block) = workspace_map.format_for_prompt() {
        base_for_map.push_str("\n\n");
        base_for_map.push_str(&block);
    }
    system_merged = Some(base_for_map);

    let ctx = ToolContext::new(workspace.clone(), cli.apply)
        .with_plan_mode(cli.plan || mode == PermissionMode::Plan)
        .with_user_asker(Arc::new(StdinUserAsker))
        .with_scope_deferred(drox_tools::ScopeDeferredHandle::new())
        .with_workspace_map(workspace_map)
        .with_drox_ignore(drox_ignore);
    let tool_hooks = drox_engine::load_tool_hooks(&workspace);
    let memory_runtime = drox_engine::MemoryRuntime {
        workspace_root: workspace,
        llm: llm.clone(),
        compaction_prompt: prompts::COMPACTION_PROMPT.to_string(),
        compaction_config: drox_engine::CompactionConfig::default(),
        notes: drox_engine::SessionNotesHandle::new(),
        model_label: cli.model.clone(),
    };
    let agent_config = AgentConfig {
        system_prompt: system_merged,
        max_iterations: cli.max_iterations,
        chat_options: ChatOptions {
            temperature: cli.temperature,
            max_tokens: cli.max_tokens,
            stop_sequences: Vec::new(),
            tools: Vec::new(),
            ..ChatOptions::default()
        },
        permissions: Some(policy),
        context: Some(ContextPolicy::for_model_context_window(num_ctx)),
        transcript,
        memory: Some(memory_runtime),
        transcript_session_id: cli.session.clone(),
        workspace_fingerprint,
        max_parallel_tool_calls: drox_engine::DEFAULT_MAX_PARALLEL_TOOL_CALLS,
        tool_hooks: if tool_hooks.is_enabled() {
            Some(tool_hooks)
        } else {
            None
        },
        run_objective: None,
    };
    let agent = Agent::new(llm, registry, ctx, agent_config);

    let mut stream = agent.run_with_history(history, prompt.to_string());
    let stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut out = stdout.lock();

    while let Some(event) = stream.next().await {
        match event.context("agent error")? {
            AgentEvent::TextDelta { text } => {
                out.write_all(text.as_bytes())?;
                out.flush()?;
            }
            AgentEvent::ToolStart {
                id,
                name,
                arguments,
            } => {
                writeln!(stderr, "\n[tool start] {name} ({id}) args={arguments}")?;
            }
            AgentEvent::ToolFinish {
                id,
                output,
                is_error,
            } => {
                let tag = if is_error { "error" } else { "ok" };
                writeln!(stderr, "[tool {tag}] {id} → {output}")?;
            }
            AgentEvent::Stop { reason, usage } => {
                writeln!(out)?;
                tracing::info!(
                    ?reason,
                    input_tokens = usage.input_tokens,
                    output_tokens = usage.output_tokens,
                    "agent terminé"
                );
                break;
            }
            AgentEvent::ContextSnip {
                tokens_freed,
                blocks_snipped,
                tokens_used_after,
            } => {
                writeln!(
                    stderr,
                    "[context snip] freed≈{tokens_freed} tok, blocks={blocks_snipped}, after≈{tokens_used_after}"
                )?;
            }
            AgentEvent::ContextCompacted {
                tokens_before,
                tokens_after,
                messages_removed,
                ..
            } => {
                writeln!(
                    stderr,
                    "[context compact] before≈{tokens_before} after≈{tokens_after} removed_msgs={messages_removed}"
                )?;
            }
            other => {
                tracing::debug!(event = ?other, "event agent non géré");
            }
        }
    }

    Ok(())
}
