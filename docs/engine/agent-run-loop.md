# Boucle agent — un « run »

Pipeline produit : **`tui_mono`** — une seule boucle dans `drox-engine` (pas de split Architecte / Exécuteur).

## Chaîne TUI (chemin principal)

```text
Composer (message utilisateur)
  → App::start_run                 drox-tui/src/app/run.rs
  → prepare_user_prompt (@, paste)
  → EngineRuntime::spawn_run       drox-tui/src/engine/bootstrap.rs
       → charge l’historique JSONL
       → Agent::new(AgentConfig { … })
       → agent.run_with_history_blocks(history, user_blocks)
  → tâche Tokio : drive_inner      drox-engine/src/agent.rs
       → compacte si besoin
       → stream LLM
       → exécute tools (toujours locaux dans le TUI)
       → nudges si le protocole dérape
       → émet AgentEvent sur un canal mpsc
  → chaque frame UI (~80 ms)
       → poll_agent_events / apply_agent_event
       → met à jour le fil (texte, tools, phases)
  → fin de run (Stop / erreur / annulation)
```

## Points de code

| Étape | Fichier |
|-------|---------|
| Boucle UI + `start_run` | `drox/crates/drox-tui/src/app/run.rs` |
| `spawn_run`, `apply_agent_event` | `drox/crates/drox-tui/src/engine/bootstrap.rs` |
| `run_with_history_blocks` / `drive_inner` | `drox/crates/drox-engine/src/agent.rs` |
| Registre tools | `drox/crates/drox-engine` → `default_tool_registry()` |

## Événements typiques (`AgentEvent`)

Sans liste exhaustive : deltas de texte, démarrage / fin d’outil, entrée de phase, compaction, arrêt. Le TUI les traduit en entrées de journal affichables.

## Chemin IDE (même `drive_inner`)

L’IDE envoie `initialize` puis `agent.run` via JSON-RPC ; le moteur stream `agent/event` puis `agent/done`.  
Voir [jsonrpc-protocol.md](jsonrpc-protocol.md) et la doc sœur IDE.
