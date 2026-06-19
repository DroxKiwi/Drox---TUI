# drox-tui — interface terminal Drox

Client **TUI propriétaire** pour le moteur Rust `drox-engine`. Réécriture inspirée
du REPL Ink du leak TypeScript, **sans** code Anthropic ni services cloud : moteur
local + LLM configurable (Ollama par défaut).

## Statut (2026-05-19)

| Composant | État |
|-----------|------|
| Bootstrap moteur (permissions, transcript, prompts) | ✅ |
| Boucle `Agent::run` + streaming `AgentEvent` | ✅ |
| Fil structuré (user, assistant, tools, phases repliables, compaction) | ✅ |
| Modal questions / permissions (`TuiUserAsker`) | ✅ previews + file d'attente multiple |
| Panneau todos live (`todo_write`) | ✅ titre `done/total` · marqueurs statut |
| Messages plan + panneau cours | ✅ fil magenta + `course_panel` titre `mastered/total` |
| Thèmes & couleurs | ✅ `/theme` (5 palettes) · `/color` (accent session) |
| Recherche | ✅ `Ctrl+F` fil (surlignage UTF-8) · `Ctrl+R` historique composer |
| Mode bash `!` | ✅ shell direct sans agent — stream live · `e` expand |
| Typeahead `@fichier` | ✅ popup unifiée `@` · `/` · `/skills` · Tab · ↑↓ |
| Menu aide `?` | ✅ popup raccourcis composer |
| Expansion `@` à l'envoi | ✅ contenu fichier annexé au prompt agent (`#L10-20`) |
| Smart paste | ✅ gros collage → `[Pasted text #N]` · expansion à l'envoi |
| Images collage | ✅ `[Image #N]` · chemin fichier · presse-papiers Win · multimodal Ollama |
| Footer composer | ✅ modèle court · mode · hints `?` `@` `/` `!` |
| Mode vim | ✅ `/vim` · Esc INSERT/NORMAL · h/j/k/l · dd/y/p/u · curseur |
| Fil messages | ✅ routeur · user/slash/image · tools groupés · cache · spinner run |
| Status line | ✅ modèle · git · tokens ↑↓ ctx · titre session · durée |
| Notices démarrage | ✅ bandeau sous header — `--apply`, plan, hooks, tips |
| Palette slash | ✅ `/` — menu commandes filtrable |
| Keybindings | ✅ `~/.drox/keybindings.json` — reload auto + `/keybindings reload` |
| Slash P2 | ✅ `/sandbox` `/review` `/security-review` `/statusline` · `/search` mémoire |
| Panneau MCP live | ✅ `mcp_panel` — serveurs `.mcp.json` |
| Onboarding + settings | ✅ modal 1er lancement · `/settings` · `/onboarding` |
| Toasts + copie rapide | ✅ toast 4s · `y` dernière réponse · `/copy` toast |
| Session rewind / export | ✅ `/rewind` · `/export` |
| Slash commands essentiels | ✅ **Sprint 9.1** — 30 slash REPL (voir `/help`) |
| Sessions (`--session`, `/resume`, `/newsession`) | ✅ + rejeu fil UI · titre onglet terminal |
| Bash fil (sortie lignes + `e` expand) | ✅ ANSI SGR · spinner · stream stdout live |
| Markdown assistant (pulldown-cmark) | ✅ titres, code · ANSI SGR · **URLs OSC 8** autolink |
| Widgets fil outils (tous outils 🦀 P1–P2) | ✅ partiel — fil structuré + viewers `e` ; LSP via extension |
| Annulation run (Esc / Ctrl+C) + file messages | ✅ |

Checklist détaillée : [`docs/TUI-REPRISE-LEAK-CHECKLIST.md`](../../../docs/TUI-REPRISE-LEAK-CHECKLIST.md).

## Build & lancement

```sh
cd drox
cargo build -p drox-tui
cargo run -p drox-tui -- --workspace /chemin/vers/projet
```

Binaire : `drox/target/debug/drox-tui(.exe)`.

Exemple Ollama local (si `~/.drox/env` pointe ailleurs) :

```sh
DROX_SERVER=http://localhost:11434 DROX_MODEL=gemma4:e4b-it-qat \
  cargo run -p drox-tui -- --workspace .
```

## Flags CLI principaux

| Flag | Rôle |
|------|------|
| `--server` / `DROX_SERVER` | URL LLM (défaut `http://localhost:11434`) |
| `--model` / `DROX_MODEL` | Modèle |
| `--workspace` | Racine tools |
| `--apply` | Écritures fichier réelles |
| `--plan` / `--mode` | Mode permission |
| `--allow` / `--ask` / `--deny` | Règles CLI |
| `--session ses_…` | Reprend un transcript |
| `--list-sessions` | Liste les `ses_*.jsonl` puis quitte |
| `-v` | Logs verbeux |

Chargement automatique de `<workspace>/.drox/env` et `~/.drox/env` via `drox_cli::env_file`.

## Commandes slash (REPL)

Sprint **9.1 complet** — tapez `/help` dans le REPL. Exemples : `/color cyan` `/copy` `/cost` `/diff` `/export` `/keybindings init` `/mcp tools` `/rename Mon run` `/rewind` `/theme` …

## Raccourcis

| Touche | Action |
|--------|--------|
| Entrée | Envoyer |
| Shift+Entrée | Nouvelle ligne |
| ↑ / ↓ | Historique composer (buffer vide) |
| `y` | Copier dernière réponse assistant (composer vide, hors vim) |
| `e` | Viewer scrollable (bash, file_read, grep, glob, web_*, diff, LSP, skill, MCP, task, plan `.drox/plan.md`) · repli phase |
| `!` | Mode bash intégré (composer vide) — ou `!cmd` one-shot |
| `@` / `/` | Suggestions unifiées · Tab compléter · `/skills` nom skill |
| `?` | Menu d'aide composer (Esc fermer) |
| `/add-dir <chemin>` | Répertoire de travail additionnel (session) |
| Ctrl+F | Recherche dans le fil (surlignage, Ctrl+n/p pour suivant/précédent) |
| Ctrl+R | Recherche dans l'historique des prompts (reverse-i-search) |
| `/` (composer vide) | Palette des commandes slash |
| Esc (run) | Annuler le run |
| Esc (idle) | Quitter |
| Ctrl+C (run) | Annuler |
| Ctrl+C ×2 (idle) | Quitter |
| Ctrl+Q | Quitter |
| `/theme` | Choisir palette (dark, light, dark-ansi, light-ansi, dim) |
| `/color <nom>` | Accent session (red, blue, cyan, … ou `default`) |
| `/keybindings` | Liste raccourcis · `/keybindings init` · `/keybindings reload` |
| PgUp / PgDown | Scroll fil |

## Architecture

```
drox-tui/
├── app/        boucle REPL, état session, file messages
├── asker/      TuiUserAsker (questions bloquantes)
├── engine/     bootstrap, commands slash async, AgentEvent → LogEntry
├── view/       fil (LogEntry), rejeu transcript, preview permission, bash_output
├── slash.rs    commandes / locales
├── terminal/   raw mode, teardown
├── ui/         layout ratatui
└── widgets/    composer, composer_suggestions, composer_help, message_log
```

Le TUI appelle **`drox_engine::Agent` directement** (pas de JSON-RPC). Configuration
système partagée avec le binaire `drox` via la lib `drox_cli`.

## Relation avec les autres clients

| Binaire | Rôle |
|---------|------|
| `drox` | CLI one-shot + JSON-RPC `--serve` (VS Code) |
| `drox-tui` | REPL terminal autonome |

Les trois consomment les mêmes crates moteur ; aucun client n’embarque de télémétrie
ni d’API Anthropic.
