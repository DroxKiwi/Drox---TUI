//! Point d'entrée binaire `drox-tui`.

use anyhow::Context;
use camino::Utf8PathBuf;
use clap::Parser;
use drox_tui::{print_sessions_list, App, AppConfig};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "drox-tui",
    version,
    about = "Drox — interface terminal (TUI) branchée sur le moteur Rust"
)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// URL du serveur LLM (Ollama par défaut).
    #[arg(long, env = "DROX_SERVER", default_value = "http://localhost:11434")]
    server: String,

    /// Modèle LLM.
    #[arg(long, env = "DROX_MODEL", default_value = "llama3.2")]
    model: String,

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .init();

    if cli.list_sessions {
        return print_sessions_list(cli.session_dir).await;
    }

    let workspace = cli
        .workspace
        .or_else(|| Utf8PathBuf::from_path_buf(std::env::current_dir().ok()?).ok())
        .context("impossible de déterminer le workspace (cwd ou --workspace)")?;

    let config = AppConfig {
        server: cli.server,
        model: cli.model,
        workspace,
        apply: cli.apply,
        plan_mode: cli.plan,
        mode: cli.mode,
        allow: cli.allow,
        ask: cli.ask,
        deny: cli.deny,
        no_settings: cli.no_settings,
        max_iterations: cli.max_iterations,
        api_key: cli.api_key,
        session: cli.session,
        session_dir: cli.session_dir,
    };

    let mut app = App::new(config);
    app.run().await
}
