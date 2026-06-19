//! Point d'entrée binaire `drox-tui`.

use anyhow::Context;
use camino::Utf8PathBuf;
use clap::Parser;
use drox_tui::{print_sessions_list, App, AppConfig};
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "drox-tui",
    version,
    about = "Drox — interface terminal (TUI) branchée sur le moteur Rust"
)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// URL du serveur LLM (sinon `/server` ou `~/.drox/tui-preferences.json`).
    #[arg(long, env = "DROX_SERVER")]
    server: Option<String>,

    /// Modèle LLM (sinon choix dans `/server`).
    #[arg(long, env = "DROX_MODEL")]
    model: Option<String>,

    /// Racine workspace exposée aux tools.
    #[arg(long, env = "DROX_WORKSPACE")]
    workspace: Option<Utf8PathBuf>,

    /// Applique réellement les écritures fichier (`file_write` / `file_edit`).
    #[arg(long)]
    apply: bool,

    /// Mode plan : pas d'écriture disque tant que non validé.
    #[arg(long)]
    plan: bool,

    /// Mode permission : `default`, `plan`, `acceptEdits`, `bypassPermissions`.
    #[arg(long)]
    mode: Option<String>,

    #[arg(long = "allow")]
    allow: Vec<String>,

    #[arg(long = "ask")]
    ask: Vec<String>,

    #[arg(long = "deny")]
    deny: Vec<String>,

    #[arg(long)]
    no_settings: bool,

    #[arg(long, default_value_t = 12)]
    max_iterations: usize,

    /// Clé API optionnelle (providers compatibles OpenAI).
    #[arg(long, env = "DROX_API_KEY")]
    api_key: Option<String>,

    /// Reprend un transcript `ses_…` existant.
    #[arg(long)]
    session: Option<String>,

    /// Répertoire des transcripts (défaut `~/.drox/sessions`).
    #[arg(long)]
    session_dir: Option<Utf8PathBuf>,

    /// Liste les sessions puis quitte.
    #[arg(long)]
    list_sessions: bool,

    /// Verbosité des logs (`-v`, `-vv`).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn init_tracing(verbose: u8) -> anyhow::Result<PathBuf> {
    let filter = match verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let log_path = dirs::home_dir()
        .map(|h| h.join(".drox").join("tui.log"))
        .context("home dir pour ~/.drox/tui.log")?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).context("creation ~/.drox")?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .context("ouverture ~/.drox/tui.log")?;
    // Ne jamais ecrire sur stderr pendant le TUI : ca corrompt l'ecran alternatif.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_writer(file)
        .with_ansi(false)
        .init();
    Ok(log_path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let _log_path = init_tracing(cli.verbose)?;

    if cli.list_sessions {
        return print_sessions_list(cli.session_dir).await;
    }

    let workspace = cli
        .workspace
        .or_else(|| Utf8PathBuf::from_path_buf(std::env::current_dir().ok()?).ok())
        .context("impossible de déterminer le workspace (cwd ou --workspace)")?;

    let config = AppConfig {
        server: cli.server.unwrap_or_default(),
        model: cli.model.unwrap_or_default(),
        workspace,
        apply: cli.apply,
        plan_mode: cli.plan,
        mode: cli.mode,
        allow: cli.allow,
        ask: cli.ask,
        deny: cli.deny,
        no_settings: cli.no_settings,
        max_iterations: cli.max_iterations.max(1),
        api_key: cli.api_key,
        num_ctx: drox_tui::engine::default_num_ctx(),
        session: cli.session,
        session_dir: cli.session_dir,
    };

    let mut app = App::new(config);
    app.run().await
}
