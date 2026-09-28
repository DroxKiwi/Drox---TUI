# Moteur Drox — documentation (TUI)

Documentation **actuelle** du moteur agent Rust et du client terminal (`drox-tui`).  
Pipeline d’orchestration : **`tui_mono`** (baseline moteur IDE **1.5.0**, produit TUI **2.0.x**).

Les dossiers `docs/2.0.x/` restent l’**historique de livraison** (plans, checklists). Ici : le fonctionnement du code tel qu’il tourne.

## Intention

Expliquer comment un message dans le terminal devient des tours LLM, des appels d’outils et des événements UI — pour forker, intégrer, ou déboguer.

## Sommaire

| Document | Contenu |
|----------|---------|
| [architecture-overview.md](architecture-overview.md) | Crates, dépendances, TUI vs CLI vs IDE |
| [agent-run-loop.md](agent-run-loop.md) | De `start_run` à `drive_inner` : tours, phases, événements |
| [tools-and-permissions.md](tools-and-permissions.md) | Palette locale, modes permission, hooks, asker TUI |
| [sessions-and-memory.md](sessions-and-memory.md) | Transcripts JSONL, prefs, memdir, `.drox/` |
| [llm-backends.md](llm-backends.md) | Ollama, OpenAI-compat, bibliothèque `/server` |
| [tui-integration.md](tui-integration.md) | Ratatui, `EngineRuntime`, slash commands |
| [jsonrpc-protocol.md](jsonrpc-protocol.md) | Chemin `drox --serve` (pour l’IDE / CLI) — pas le chemin TUI principal |
| [glossary.md](glossary.md) | Termes stables (`tui_mono`, `AgentEvent`, `EngineRuntime`, …) |

## Carte mentale (1 minute)

```text
Toi + repo  ↔  drox-tui  (Ratatui, même process)
                 ↕  appels Rust directs
              drox-engine  (drive_inner, tools, permissions)
                 ↕  HTTP
              Ollama / endpoint OpenAI-compat
                 ↕
              modèle (Qwen, Gemma, …)
```

Contrairement à l’IDE, **pas de JSON-RPC** sur le chemin principal : le TUI **embarque** le moteur.

Un run : composer → `spawn_run` → `drive_inner` (LLM → outils → …) → `AgentEvent` → fil UI → fin de run.

## Code source

| Zone | Chemin |
|------|--------|
| Workspace Rust | [`drox/`](../../drox/) |
| Boucle agent | `drox/crates/drox-engine/src/agent.rs` |
| Client TUI | `drox/crates/drox-tui/` |
| Bootstrap / events | `drox/crates/drox-tui/src/engine/bootstrap.rs` |
| Serveur RPC (IDE) | `drox/crates/drox-cli/src/jsonrpc/` |

## Sœur IDE

Même cerveau, autre peau : [Drox IDE — docs/engine](https://github.com/DroxKiwi/Drox---IDE/tree/main/docs/engine).
