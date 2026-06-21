# Checklist — Ligne 2.0.5

## Architecture & externalisation

- [ ] Crate `drox-observe` créé (sans dépendance ratatui)
- [ ] `ObserveEvent` + feature flags documentés
- [ ] Hook moteur opt-in (pas de régression `drox-engine` tests)
- [ ] Fiches [`docs/features/`](../features/README.md) F01–F05 validées

## M0 — Shell multi-pane (F01)

- [ ] `PaneManager` + toggles masquer/afficher
- [ ] Focus Tab · Esc restore
- [ ] Fallback terminal étroit
- [ ] Adaptateur TUI observe (stub)

## M1 — Changements + beats (F04, F02)

- [ ] `RunChangesSnapshot` live pendant run
- [ ] Beat ID mécaniques sur tools fichier
- [ ] Panneau diff : liste j/k + clic
- [ ] Corrélation couleur fil ↔ panneau changements

## M2 — Contexte & carte (F03, F05)

- [ ] `ContextManifestBuilder` post-tour LLM
- [ ] Panneau carte : arbre ● ○ ◇ ▲
- [ ] Corrélation beat sur 3 panes
- [ ] Scroll auto nœud actif

## M3 — Finitions

- [ ] Diff inline fil (optionnel)
- [ ] i18n FR/EN panes
- [ ] Prefs layout mémorisé
- [ ] Non-régression overlay 2.0.4

## M4 — Portabilité IDE

- [ ] `observe_schema_version` + exemples JSON
- [ ] Spec relay RPC (doc)
- [ ] Matrice port par feature

## Release

- [x] Bump version 2.0.5 (workspace)
- [ ] RELEASE_NOTES OR
- [ ] Merge → `main`
