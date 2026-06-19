# Drox — agent local en terminal

**Drox** est un moteur d’agent IA écrit en Rust, avec une interface terminal (**TUI**) complète. LLM local (Ollama par défaut), outils fichiers/shell/MCP, permissions et sessions — sans service cloud obligatoire.

## Démarrage rapide

```sh
cd drox
cargo run -p drox-tui -- --workspace /chemin/vers/projet
```

Avec Ollama :

```sh
DROX_SERVER=http://localhost:11434 DROX_MODEL=llama3.2 \
  cargo run -p drox-tui -- --workspace .
```

Config auto : `<workspace>/.drox/env` et `~/.drox/env`. Binaire : `drox/target/debug/drox-tui`.

| Flag / env | Rôle |
|---|---|
| `--server` / `DROX_SERVER` | URL LLM (défaut `http://localhost:11434`) |
| `--model` / `DROX_MODEL` | Modèle |
| `--workspace` | Racine des outils |
| `--apply` | Autorise les écritures disque |
| `--plan` / `--mode` | Mode permission (plan, acceptEdits…) |
| `--session ses_…` | Reprend une session |
| `--list-sessions` | Liste les sessions puis quitte |

## Interface TUI (`drox-tui`)

REPL terminal branché directement sur `drox-engine` (ratatui). ~30 commandes slash — tapez `/help` dans le REPL.

### Agent & fil de conversation

- Boucle agent streaming (`Agent::run` → événements temps réel)
- Fil structuré : messages user/assistant, appels d’outils groupés, phases repliables, compaction
- Markdown assistant (titres, blocs code, liens cliquables OSC 8)
- Annulation de run (Esc / Ctrl+C) et file de messages
- Spinner de run, cache fil, rejeu transcript à la reprise de session

### Composer & saisie

- Mode **vim** (`/vim`) — INSERT/NORMAL, h/j/k/l, dd/y/p/u
- Typeahead unifié `@fichier` · `/` · `/skills` (Tab, ↑↓)
- Expansion `@` à l’envoi (contenu fichier + plages `#L10-20`)
- **Smart paste** — gros texte → `[Pasted text #N]`, expansion à l’envoi
- Collage **d’images** → `[Image #N]` (fichier, presse-papiers Windows, multimodal Ollama)
- Mode **bash** `!` — shell direct sans agent, stream live ; `!cmd` one-shot
- Palette slash `/` (composer vide) — menu filtrable
- Menu d’aide `?` — raccourcis du composer

### Viewers & outils (`e` pour expand)

Sorties scrollables pour bash, `file_read`, grep, glob, web fetch/search, diff, LSP, skills, MCP, tasks, plan (`.drox/plan.md`). Bash avec ANSI SGR et stream stdout live.

### Panneaux & statut

- **Todos** live (`todo_write`) — compteur done/total
- **Plan / cours** — messages plan + panneau mastered/total
- **MCP** live — serveurs `.mcp.json`
- Status line : modèle, branche git, tokens contexte, titre session, durée
- Notices au démarrage (`--apply`, plan, hooks, tips)
- Toasts (4 s), copie rapide `y` ou `/copy`

### Permissions & questions

- Modal questions / permissions avec previews
- File d’attente multiple, mode plan, sandbox (`/sandbox`)

### Recherche & navigation

- `Ctrl+F` — recherche dans le fil (surlignage UTF-8, Ctrl+n/p)
- `Ctrl+R` — reverse-i-search dans l’historique des prompts
- PgUp / PgDown — scroll du fil

### Sessions & persistance

- `--session`, `/resume`, `/newsession`, `/rename`
- `/rewind` — retour arrière dans la conversation
- `/export` — export transcript
- Titre d’onglet terminal, stats UI persistées

### Personnalisation

- 5 thèmes (`/theme`) : dark, light, dark-ansi, light-ansi, dim
- Accent session (`/color cyan`…)
- Keybindings `~/.drox/keybindings.json` — reload auto, `/keybindings init|reload`
- `/settings`, `/onboarding` (modal premier lancement)
- `/statusline`, `/add-dir`, `/doctor`, `/cost`, `/diff`, `/mcp tools`, `/review`, `/security-review`…

## Moteur Rust (`drox/`)

Workspace de **12 crates** — le TUI et le CLI partagent le même moteur :

| Crate | Rôle |
|---|---|
| `drox-engine` | Boucle agent, orchestration, streaming |
| `drox-tools` | FileRead/Write/Edit, Grep, Glob, Bash, WebFetch, MCP, todos, skills… |
| `drox-llm` | Client LLM (Ollama-first), streaming, retry |
| `drox-permissions` | allow / ask / deny, plan mode, sandbox |
| `drox-context` | Tokens, compaction, snip |
| `drox-session` | Transcripts JSONL, mémoire, reprise |
| `drox-mcp` | Protocole MCP (`rmcp`), OAuth |
| `drox-bash` | Parser Bash/PowerShell, classification sécurité |
| `drox-hooks` | Hooks d’événements |
| `drox-cli` | Binaire `drox` — CLI one-shot + JSON-RPC stdio |
| `drox-tui` | Binaire `drox-tui` — REPL terminal |
| `drox-types` | Types partagés |

```sh
cd drox
cargo check    # vérification
cargo build    # build debug
cargo test     # tests
```

## Documentation

| Document | Contenu |
|---|---|
| [`drox/crates/drox-tui/README.md`](drox/crates/drox-tui/README.md) | Détails TUI, raccourcis complets |
| [`drox/README.md`](drox/README.md) | Architecture moteur, conventions |
| [`docs/PLAN-MOTEUR-RUST.md`](docs/PLAN-MOTEUR-RUST.md) | Plan de développement |
| [`docs/PROTOCOLE-JSONRPC.md`](docs/PROTOCOLE-JSONRPC.md) | Contrat moteur ↔ clients |
| [`docs/TUI-REPRISE-LEAK-CHECKLIST.md`](docs/TUI-REPRISE-LEAK-CHECKLIST.md) | Checklist fonctionnelle TUI |

## Licence

MIT — voir [`drox/Cargo.toml`](drox/Cargo.toml).
