# Glossaire

| Terme | Sens |
|-------|------|
| **`tui_mono`** | Pipeline mono-boucle agent (depuis moteur 1.5.0). Pas de split Architecte/Exécuteur. |
| **`drive_inner`** | Cœur de la boucle LLM ↔ tools dans `drox-engine`. |
| **`AgentEvent`** | Événement streamé pendant un run (texte, tool, phase, stop…). |
| **`EngineRuntime`** | Façade TUI : bootstrap, spawn_run, poll, tool_context. |
| **`RemoteTool`** | Outil exécuté côté client IDE via `tool/exec` — **absent** du chemin TUI. |
| **Session `ses_*`** | Identifiant + transcript JSONL sous `~/.drox/sessions/`. |
| **Memdir** | Fichiers `MEMORY.md` / `DROX.md` injectés dans le prompt système. |
| **`/server`** | Dialogue de connexion LLM (local / cloud que **tu** configures). |
| **`/update`** | Vérif MAJ opt-in via `releases/latest.json`. |
| **OR** | Ancien dépôt miroir Releases — **obsolète** ; canal = ce dépôt. |
