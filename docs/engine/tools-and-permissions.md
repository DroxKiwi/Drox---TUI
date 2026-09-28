# Outils et permissions

## Palette

Le TUI utilise **`default_tool_registry()`** (`drox-engine`) : outils **locaux** uniquement (lecture / écriture fichiers, bash, grep, MCP, …).

Pas de `RemoteTool` : contrairement à l’IDE, il n’y a pas de round-trip `tool/exec` vers un workbench.

Contexte d’exécution : `EngineRuntime::tool_context` dans `drox-tui/src/engine/bootstrap.rs` (workspace, plan mode, asker TUI, `.droxignore`, carte workspace…).

## Permissions

Moteur : crate `drox-permissions` (allow / ask / deny, modes).

Sources de règles (ordre conceptuel) :

- `~/.drox/settings.json`
- `<workspace>/.drox/settings.json`
- `<workspace>/.drox/settings.local.json`
- flags CLI `--allow` / `--ask` / `--deny`
- modes `--mode`, `--plan`, slash `/plan`

Questions interactives : `drox-tui/src/asker/` (`AskCoordinator`, modales) — équivalent UX de `user/ask` côté IDE.

## Hooks

Fichier projet : `<workspace>/.drox/hooks.json`  
Chargement : `drox_hooks` / `drox_engine::load_tool_hooks`. Reload possible via slash `/hooks`.

## MCP

Config type `.mcp.json` ; panneau live côté TUI (`engine/mcp.rs`). Les outils MCP apparaissent dans le registre comme les autres, sous permissions.

## Bash hors run

Préfixe `!` dans le composer : exécution shell utilisateur via le tool `bash`, hors boucle agent (`execute_user_bash`).
