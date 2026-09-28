# Intégration TUI

Comment le terminal Ratatui branche le moteur **in-process**.

## Démarrage

| Étape | Fichier |
|-------|---------|
| `main` (clap, logs) | `drox/crates/drox-tui/src/main.rs` |
| `App::run` | `drox/crates/drox-tui/src/app/run.rs` |
| `EngineRuntime::bootstrap` | `drox/crates/drox-tui/src/engine/bootstrap.rs` |

Ordre typique :

1. Charger `.env` workspace / home
2. Résoudre LLM (prefs + CLI)
3. `default_tool_registry()`, permissions, ignore, system prompt
4. Ouvrir / créer session `ses_*`
5. Boucle ~80 ms : poll events → draw → input

## Surface utilisateur

- Composer + fil chronologique
- Slash commands (`/server`, `/diff`, `/update`, `/plan`, `/resume`, …)
- Overlays (diff, dialogue IA, permissions)
- i18n FR/EN (`drox-tui/src/i18n/`)

## Différence IDE

| | TUI | IDE |
|---|-----|-----|
| Process | Un binaire | Electron + `drox --serve` |
| Transport | Rust direct | JSON-RPC NDJSON |
| Tools | Locaux | Locaux + RemoteTool |
| MAJ | `releases/latest.json` (ce dépôt) | `stable/latest.json` |

## Checklist crate

Voir [`drox/crates/drox-tui/README.md`](../../drox/crates/drox-tui/README.md).
