# Drox — moteur agent (Rust)

Binaire `drox-cli` autonome qui implémente la boucle agent. Communique via stdio (CLI texte + JSON-RPC à terme) avec un client UI (CLI, extension VS Code, ou tout autre consommateur).

## Statut

**Phase 1 — Rewrite Rust en cours.** Le moteur TypeScript dans `../src/` est la spécification exécutable. Voir `../docs/PLAN-MOTEUR-RUST.md` et `../docs/INVENTAIRE-NOYAU-MOTEUR.md` pour le plan détaillé.

## Architecture

10 crates, dépendances unidirectionnelles :

```
drox-cli  →  drox-engine
              │
              ├── drox-tools
              │   ├── drox-bash
              │   ├── drox-mcp
              │   └── drox-permissions
              │
              ├── drox-llm
              ├── drox-context
              └── drox-session

drox-tui  →  drox-engine   (client terminal — en cours)

(drox-types est consommé par toutes les autres crates)
```

| Crate | Rôle |
|---|---|
| `drox-types` | Types partagés, schémas serde, erreurs |
| `drox-llm` | Client LLM (Ollama-first), streaming, retry |
| `drox-tools` | Implémentations des tools (FileRead, Bash, MCP, etc.) |
| `drox-mcp` | Wrapper MCP (au-dessus de `rmcp`), OAuth, config |
| `drox-bash` | Parser Bash + PowerShell, AST, classification |
| `drox-permissions` | Modèle de permissions, plan mode, sandbox |
| `drox-context` | Token estimation, compaction, gestion du contexte long |
| `drox-session` | Storage JSONL, transcript, resume, mémoire fichier |
| `drox-engine` | Boucle agent, orchestration tools, streaming |
| `drox-cli` | Binaire principal, parsing CLI, JSON-RPC |
| `drox-tui` | Interface terminal (TUI) propriétaire — `ratatui` |

## Build

```sh
cd drox
cargo check       # vérification rapide
cargo build       # build debug
cargo test        # tests
cargo fmt         # formatage
cargo clippy      # lints
```

## Conventions

- Pas d'`unsafe` (interdit par lint workspace).
- Pas de panic en code de prod (clippy nursery + tests).
- Erreurs typées avec `thiserror` au niveau lib, `anyhow` au niveau binaire.
- Async partout, runtime `tokio`.
- `tracing` pour les logs (pas de `println!`/`eprintln!` hors `drox-cli`).
