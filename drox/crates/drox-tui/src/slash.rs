//! Commandes slash locales (terminal Drox uniquement).

mod appearance;
mod config;
mod doctor;
mod hooks;
mod init;
mod mcp;
mod plan;
pub(crate) mod palette;
mod skills;

use camino::Utf8Path;

use crate::app::AppState;
use crate::engine::EngineRuntime;
use crate::i18n::{self, keys_p1 as k};

pub use appearance::{apply_theme, handle_color, handle_theme};
pub use config::handle_config;
pub use doctor::handle_doctor;
pub use hooks::handle_hooks;
pub use init::handle_init;
pub use mcp::handle_mcp;
pub use plan::handle_plan;
pub use skills::handle_skills;
pub use palette::{filter_entries, SlashPaletteEntry, ENTRIES as SLASH_PALETTE_ENTRIES};

/// Résultat d'une commande `/…`.
pub enum SlashOutcome {
    /// Continuer la boucle sans lancer le moteur.
    Handled,
    /// Quitter l'application.
    Quit,
    /// Texte à envoyer au moteur (après `/` non reconnu traité comme message).
    RunPrompt(String),
    /// Nouvelle session (nouveau transcript).
    NewSession,
    /// Reprendre une session `ses_…`.
    ResumeSession(String),
    /// Compaction LLM du transcript courant.
    Compact,
    /// Liste mémoire archivée.
    MemoryList { limit: usize },
    /// Lit une session mémoire par slug.
    MemoryRead { slug: String },
    /// Affiche règles de permission.
    Permissions,
    /// Bascule / affiche mode plan.
    Plan { args: String },
    /// Usage contexte (tokens).
    Context,
    /// Hooks Pre/Post tool.
    Hooks { args: String },
    /// Configuration runtime et fichiers.
    Config,
    /// Diagnostic santé environnement.
    Doctor,
    /// Serveurs MCP (list, tools, resources, ping).
    Mcp { args: String },
    /// Skills locaux `.drox/skills/`.
    Skills { args: String },
    /// Scaffold init workspace.
    Init,
    /// Tokens / usage session.
    Cost,
    /// Git diff workspace.
    Diff,
    /// Fichiers vus dans le fil.
    Files,
    /// Branche git courante.
    Branch,
    /// Sélecteur rembobinage transcript.
    Rewind,
    /// Export texte brut du transcript.
    Export { filename: String },
    /// Sélecteur thème TUI.
    ThemePicker,
    /// Affiche / initialise keybindings.
    Keybindings { args: String },
    /// Guide intégration terminal + keybindings.
    TerminalSetup,
    /// Titre personnalisé session (`ses_*.meta.json`).
    Rename { title: Option<String> },
    /// Copie réponse assistant (`/copy [N]`).
    Copy { age: usize },
    /// Bascule mode vim du composer.
    ToggleVim,
    /// Recherche mémoire longue locale.
    MemorySearch { query: String, limit: usize },
    /// Affiche la barre de statut TUI.
    Statusline,
    /// Réglages TUI (`~/.drox/tui-preferences.json`).
    Settings,
    /// Bascule animations UI.
    SettingsAnimations { enabled: bool },
    /// Bascule capture souris.
    SettingsMouse { enabled: bool },
    /// Modal connexion serveur IA.
    AiServer,
    /// Modal changement workspace.
    Workspace { initial: Option<String> },
    /// Modal onboarding.
    Onboarding,
    /// Bascule langue UI (`/language fr|en`).
    SettingsLocale { locale: crate::i18n::UiLocale },
}

/// Action slash différée (appels async LLM / disque).
#[derive(Debug, Clone)]
pub enum PendingSlash {
    Compact,
    MemoryList { limit: usize },
    MemoryRead { slug: String },
    Permissions,
    Context,
    Doctor,
    Mcp { args: String },
    Skills { args: String },
    Cost,
    Diff,
    Files,
    Branch,
    Init,
    Rewind,
    Export { filename: String },
    RewindApply { message_index: usize },
    Keybindings { args: String },
    Rename { title: Option<String> },
    /// Copie réponse assistant (`/copy [N]`).
    Copy { age: usize },
    /// Application d'un choix du sélecteur `/copy`.
    CopyApply {
        full_text: String,
        blocks: Vec<crate::engine::copy_cmd::CopyCodeBlock>,
        choice: crate::engine::copy_cmd::CopyChoice,
        clipboard: bool,
    },
    /// Exécution shell mode `!`.
    BashExec { command: String },
    MemorySearch { query: String, limit: usize },
    Statusline,
    /// Applique le modèle choisi dans `/server`.
    ApplyAiServer { index: usize },
    /// Applique le workspace validé dans `/workspace`.
    ApplyWorkspace { path: camino::Utf8PathBuf },
}

pub fn handle_slash(input: &str, state: &mut AppState, runtime: &EngineRuntime) -> SlashOutcome {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return SlashOutcome::RunPrompt(trimmed.to_string());
    }

    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    let cmd = parts.first().copied().unwrap_or("/help").to_ascii_lowercase();
    match cmd.as_str() {
        "/help" | "/?" => {
            state.push_system(i18n::t(k::SLASH_HELP_BODY));
            SlashOutcome::Handled
        }
        "/clear" => {
            state.clear_transcript();
            state.push_system(i18n::t(k::SLASH_MSG_CLEAR));
            SlashOutcome::Handled
        }
        "/new" | "/newsession" => SlashOutcome::NewSession,
        "/resume" => {
            let id = parts.get(1).copied().unwrap_or("").trim();
            if id.is_empty() {
                state.push_system(i18n::t(k::SLASH_MSG_RESUME_USAGE));
                SlashOutcome::Handled
            } else {
                SlashOutcome::ResumeSession(id.to_string())
            }
        }
        "/exit" | "/quit" => SlashOutcome::Quit,
        "/status" => {
            state.push_system(format!(
                "workspace={} | modèle={} | mode={} | plan={} | session={} | apply={}",
                runtime.workspace,
                runtime.model_label(),
                runtime.permission_mode().short_title(),
                runtime.plan_mode(),
                runtime.session_id(),
                runtime.apply_fs_writes,
            ));
            SlashOutcome::Handled
        }
        "/model" | "/server" => SlashOutcome::AiServer,
        "/session" => {
            state.push_system(format!(
                "Session : {} | Transcript : {} (id {})",
                if state.session_title.is_empty() {
                    runtime.session_id()
                } else {
                    state.session_title.clone()
                },
                runtime.transcript_path(),
                runtime.session_id()
            ));
            SlashOutcome::Handled
        }
        "/sessions" => {
            match list_sessions_sync(&runtime.sessions_dir) {
                Ok(lines) => {
                    if lines.is_empty() {
                        state.push_system(i18n::t(k::SLASH_MSG_NO_SESSIONS));
                    } else {
                        for line in lines {
                            state.push_system(line);
                        }
                    }
                }
                Err(e) => state.push_system(i18n::tf(k::SLASH_MSG_SESSIONS_LIST_ERR, &e.to_string())),
            }
            SlashOutcome::Handled
        }
        "/compact" => SlashOutcome::Compact,
        "/search" => {
            let query = trimmed.strip_prefix("/search").unwrap_or("").trim();
            if query.is_empty() {
                state.push_system(i18n::t(k::SLASH_MSG_MEMORY_SEARCH_USAGE));
                SlashOutcome::Handled
            } else {
                SlashOutcome::MemorySearch {
                    query: query.to_string(),
                    limit: 8,
                }
            }
        }
        "/memory" => match parts.get(1).copied() {
            Some("search") | Some("find") => {
                let query = parts[2..].join(" ");
                if query.trim().is_empty() {
                    state.push_system(i18n::t(k::SLASH_MSG_MEMORY_SEARCH_SHORT));
                    SlashOutcome::Handled
                } else {
                    SlashOutcome::MemorySearch {
                        query,
                        limit: 8,
                    }
                }
            }
            None => SlashOutcome::MemoryList { limit: 10 },
            Some(arg) => {
                if let Ok(limit) = arg.parse::<usize>() {
                    SlashOutcome::MemoryList { limit }
                } else {
                    SlashOutcome::MemoryRead {
                        slug: arg.to_string(),
                    }
                }
            }
        },
        "/permissions" | "/perms" => SlashOutcome::Permissions,
        "/plan" => {
            let args = trimmed
                .strip_prefix("/plan")
                .unwrap_or("")
                .trim()
                .to_string();
            SlashOutcome::Plan { args }
        }
        "/context" | "/ctx" => SlashOutcome::Context,
        "/hooks" => {
            let args = trimmed
                .strip_prefix("/hooks")
                .unwrap_or("")
                .trim()
                .to_string();
            SlashOutcome::Hooks { args }
        }
        "/config" => SlashOutcome::Config,
        "/doctor" => handle_doctor(),
        "/mcp" => {
            let args = trimmed
                .strip_prefix("/mcp")
                .unwrap_or("")
                .trim()
                .to_string();
            handle_mcp(&args)
        }
        "/skills" => {
            let args = trimmed
                .strip_prefix("/skills")
                .unwrap_or("")
                .trim()
                .to_string();
            handle_skills(&args)
        }
        "/cost" | "/stats" | "/usage" => SlashOutcome::Cost,
        "/branch" => SlashOutcome::Branch,
        "/rewind" => SlashOutcome::Rewind,
        "/export" => {
            let filename = trimmed
                .strip_prefix("/export")
                .unwrap_or("")
                .trim()
                .to_string();
            SlashOutcome::Export { filename }
        }
        "/theme" => {
            let args = trimmed
                .strip_prefix("/theme")
                .unwrap_or("")
                .trim();
            handle_theme(args, state)
        }
        "/color" => {
            let args = trimmed
                .strip_prefix("/color")
                .unwrap_or("")
                .trim();
            handle_color(args, state)
        }
        "/keybindings" => {
            let args = trimmed
                .strip_prefix("/keybindings")
                .unwrap_or("")
                .trim()
                .to_string();
            SlashOutcome::Keybindings { args }
        }
        "/terminal-setup" | "/terminalsetup" => SlashOutcome::TerminalSetup,
        "/vim" => SlashOutcome::ToggleVim,
        "/rename" => {
            let title = trimmed
                .strip_prefix("/rename")
                .unwrap_or("")
                .trim();
            SlashOutcome::Rename {
                title: if title.is_empty() {
                    None
                } else {
                    Some(title.to_string())
                },
            }
        }
        "/copy" => {
            let arg = trimmed
                .strip_prefix("/copy")
                .unwrap_or("")
                .trim();
            let age = if arg.is_empty() {
                0
            } else {
                match arg.parse::<usize>() {
                    Ok(n) if n >= 1 => n - 1,
                    _ => {
                        state.push_system(i18n::tf(k::SLASH_MSG_COPY_USAGE, arg));
                        return SlashOutcome::Handled;
                    }
                }
            };
            SlashOutcome::Copy { age }
        }
        "/output-style" | "/outputstyle" => {
            state.push_system(
                "Commande dépréciée — utilisez /config pour le style de sortie (effet au prochain run).",
            );
            SlashOutcome::Handled
        }
        "/add-dir" | "/adddir" => {
            let args = trimmed
                .strip_prefix("/add-dir")
                .or_else(|| trimmed.strip_prefix("/adddir"))
                .unwrap_or("")
                .trim();
            crate::engine::add_dir_cmd::handle_add_dir(args, state, runtime)
        }
        "/workspace" | "/ws" => {
            let args = trimmed
                .strip_prefix("/workspace")
                .or_else(|| trimmed.strip_prefix("/ws"))
                .unwrap_or("")
                .trim();
            SlashOutcome::Workspace {
                initial: if args.is_empty() {
                    None
                } else {
                    Some(args.to_string())
                },
            }
        }
        "/diff" => SlashOutcome::Diff,
        "/files" => SlashOutcome::Files,
        "/init" => {
            let args = trimmed
                .strip_prefix("/init")
                .unwrap_or("")
                .trim()
                .to_string();
            handle_init(&args)
        }
        "/sandbox" => {
            for line in crate::engine::format_sandbox_lines(runtime) {
                state.push_system(line);
            }
            SlashOutcome::Handled
        }
        "/review" => {
            let args = trimmed
                .strip_prefix("/review")
                .unwrap_or("")
                .trim();
            let mut prompt = crate::engine::REVIEW_AGENT_PROMPT.to_string();
            if !args.is_empty() {
                prompt.push_str(&format!("\n\nNuméro ou branche PR : {args}"));
            }
            SlashOutcome::RunPrompt(prompt)
        }
        "/security-review" | "/securityreview" => {
            SlashOutcome::RunPrompt(crate::engine::SECURITY_REVIEW_AGENT_PROMPT.to_string())
        }
        "/statusline" | "/status-line" => {
            let sub = trimmed
                .split_whitespace()
                .nth(1)
                .map(|s| s.to_ascii_lowercase());
            if matches!(sub.as_deref(), Some("run") | Some("setup")) {
                SlashOutcome::RunPrompt(crate::engine::STATUSLINE_SETUP_PROMPT.to_string())
            } else {
                SlashOutcome::Statusline
            }
        }
        "/settings" => match parts.as_slice() {
            [_, "animations", "off" | "false" | "0"] => SlashOutcome::SettingsAnimations {
                enabled: false,
            },
            [_, "animations", "on" | "true" | "1"] | [_, "animations"] => {
                SlashOutcome::SettingsAnimations { enabled: true }
            }
            [_, "mouse", "off" | "false" | "0"] => SlashOutcome::SettingsMouse { enabled: false },
            [_, "mouse", "on" | "true" | "1"] | [_, "mouse"] => {
                SlashOutcome::SettingsMouse { enabled: true }
            }
            [_, "language", code] => {
                if let Some(locale) = crate::i18n::UiLocale::parse(code) {
                    SlashOutcome::SettingsLocale { locale }
                } else {
                    state.push_system(crate::i18n::t(crate::i18n::keys::LANGUAGE_USAGE));
                    SlashOutcome::Handled
                }
            }
            [_, "print"] => {
                let prefs = crate::engine::preferences::load_preferences();
                for line in crate::engine::preferences::format_settings_lines(&prefs) {
                    state.push_system(line);
                }
                SlashOutcome::Handled
            }
            _ => SlashOutcome::Settings,
        },
        "/language" => match parts.get(1) {
            Some(code) => {
                if let Some(locale) = crate::i18n::UiLocale::parse(code) {
                    SlashOutcome::SettingsLocale { locale }
                } else {
                    state.push_system(crate::i18n::t(crate::i18n::keys::LANGUAGE_USAGE));
                    SlashOutcome::Handled
                }
            }
            None => {
                state.push_system(crate::i18n::t(crate::i18n::keys::LANGUAGE_USAGE));
                SlashOutcome::Handled
            }
        },
        "/onboarding" => SlashOutcome::Onboarding,
        _ => {
            state.push_system(i18n::tf2(
                k::SLASH_MSG_UNKNOWN_CMD,
                &cmd,
                i18n::t(k::SLASH_HELP_BODY),
            ));
            SlashOutcome::Handled
        }
    }
}

fn list_sessions_sync(dir: &Utf8Path) -> anyhow::Result<Vec<String>> {
    use drox_session::{display_title, SessionMeta};
    use drox_types::SessionId;

    let mut entries = Vec::new();
    let read = match std::fs::read_dir(dir.as_std_path()) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    for ent in read {
        let ent = ent?;
        let path = ent.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("ses_") || !name.ends_with(".jsonl") {
            continue;
        }
        let meta = ent.metadata()?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        entries.push((name.trim_end_matches(".jsonl").to_string(), modified, meta.len()));
    }
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(entries
        .into_iter()
        .take(12)
        .map(|(id, _, size)| {
            let sid = SessionId::from_string(id.clone());
            let meta_path = drox_session::session_meta_path(dir, &sid);
            let title = std::fs::read(&meta_path.as_std_path())
                .ok()
                .and_then(|b| serde_json::from_slice::<SessionMeta>(&b).ok())
                .map(|m| display_title(&id, Some(&m)))
                .unwrap_or_else(|| id.clone());
            if title == id {
                format!("{id} — {size} o")
            } else {
                format!("{title} ({id}) — {size} o")
            }
        })
        .collect())
}
