# Architecture — vue d’ensemble

## Rôle du moteur

Dans le **TUI**, `drox-engine` tourne **dans le même processus** que l’UI Ratatui. Il choisit les tours LLM, décide quels outils appeler, gère permissions / contexte / session, et pousse des `AgentEvent` vers le fil terminal.

Le binaire `drox` (`drox-cli`) reste disponible pour le chemin IDE : `drox --serve` (JSON-RPC stdio). Le TUI ne l’utilise pas pour un run normal.

## Workspace Rust

Racine : `drox/` (Cargo workspace, version produit dans `[workspace.package]`).

```text
drox-types
    ↑
drox-llm · drox-bash · drox-permissions · drox-context
drox-session · drox-hooks · drox-mcp
    ↑
drox-tools
    ↑
drox-engine          ← boucle Agent::run / drive_inner
    ↑
drox-cli             ← binaire `drox` (--serve, CLI)
drox-tui             ← client terminal (même Agent, in-process)
```

Dépendances **unidirectionnelles** : les feuilles ne importent pas `drox-engine`.

| Crate | Rôle |
|-------|------|
| `drox-types` | Types partagés, schémas serde, erreurs |
| `drox-llm` | Clients LLM (Ollama, OpenAI-compat), streaming |
| `drox-tools` | Implémentations d’outils + `ToolRegistry` |
| `drox-mcp` | Serveurs MCP (`rmcp`), outils `mcp__*` |
| `drox-bash` | Analyse / classification shell |
| `drox-permissions` | Allow / ask / deny, modes |
| `drox-context` | Budget tokens, compaction |
| `drox-session` | Transcripts JSONL, métadonnées, mémoire fichier |
| `drox-hooks` | Hooks pre/post tool (`.drox/hooks.json`) |
| `drox-engine` | Orchestration mono-boucle (`tui_mono`) |
| `drox-cli` | Binaire serveur / CLI (chemin IDE) |
| `drox-tui` | UI terminal + `EngineRuntime` |

Voir aussi [`drox/README.md`](../../drox/README.md).

## Trois clients, un cerveau

| Client | Process | Transport |
|--------|---------|-----------|
| **drox-tui** | UI + moteur ensemble | Appels Rust (`spawn_run`) |
| **Drox IDE** | Electron + `drox --serve` | NDJSON JSON-RPC |
| **drox** (CLI) | One-shot ou `--serve` | Stdio / JSON-RPC |

## Couches TUI (rappel produit)

Au-dessus du moteur, le TUI ajoute :

1. **Shell Ratatui** — composer, fil, overlays (`/diff`, `/server`, …)
2. **Corrélation run** — mapping `AgentEvent` → `LogEntry` UI
3. **Prefs & connexions** — `~/.drox/tui-preferences.json`, bibliothèque LLM

Détail roadmap UI : [`docs/2.0.6/ARCHITECTURE-FEATURES.md`](../2.0.6/ARCHITECTURE-FEATURES.md).
