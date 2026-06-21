# Ligne produit `2.0.5` — Drox TUI · Poste de pilotage multi-pane

**Version produit** : `2.0.5` (dev)  
**Branche Git** : `2.0.5`  
**Release OR publique** : `2.0.4` (Windows)  
**Moteur** : dérivé du **moteur agent Drox IDE `1.5.0`** — **couche observe** nouvelle, moteur legacy touch minimal  
**Prédécesseur** : [`2.0.4`](../2.0.4/README.md) — diff overlay (clôturée)

---

## Objectif

Transformer le TUI en **poste de pilotage agent** : jusqu’à **3 vues simultanées** (fil · carte contexte · changements+diff), panneaux masquables, corrélés par **beat IDs** colorés (A1, A2…).

La release introduit une **architecture code externalisée** (crate `drox-observe`, modules par feature) pour limiter les effets de bord moteur et faciliter le port vers Drox IDE.

```mermaid
flowchart LR
  FIL["Fil agent"]
  MAP["Carte contexte LLM"]
  CHG["Changements + diff"]
  FIL --- MAP
  MAP --- CHG
  FIL -.->|"A12 couleur"| CHG
```

---

## Documents

| Document | Rôle |
|---|---|
| [PLAN-MULTI-PANE.md](PLAN-MULTI-PANE.md) | Vision UX, jalons M0–M4 |
| [ARCHITECTURE-FEATURES.md](ARCHITECTURE-FEATURES.md) | Découplage **code** moteur / `drox-observe` / TUI |
| [FEATURES.md](FEATURES.md) | Index specs F01–F05 |
| [CHECKLIST.md](CHECKLIST.md) | Suivi implémentation |
| [PLAN-DIFF-INLINE.md](PLAN-DIFF-INLINE.md) | Sous-feature M3 (diff fil — historique) |

---

## Features 2.0.5

| ID | Feature | Spec | Module code |
|---|---|---|---|
| F01 | Multi-pane shell | [F01](F01-multi-pane-shell.md) | `drox-tui/panes/` |
| F02 | Beat ID | [F02](F02-beat-id-correlation.md) | `drox-observe/beat/` |
| F03 | Context manifest | [F03](F03-context-manifest.md) | `drox-observe/manifest/` |
| F04 | Changements + diff | [F04](F04-run-changes-panel.md) | `drox-observe/changes/` |
| F05 | Carte workspace | [F05](F05-workspace-map-view.md) | `drox-observe/map_view/` |

---

## Périmètre hors 2.0.5

- Graphe mermaid / layout graphique auto
- Édition fichier depuis panneau diff
- Relay RPC observe vers IDE (spec M4, impl IDE séparée)
- Code signing → [2.0.6](../2.0.6/README.md)

---

## Réutilisation 2.0.4

| Existant | Usage 2.0.5 |
|---|---|
| `diff_render`, `lines_viewer` | Panneau changements (F04) |
| `RunFileChange` | Base `RunChangesSnapshot` → observe |
| `WorkspaceMapStore` | Structure ; F03 pour état contextuel |
| Overlay `/diff` | Conservé ; fallback terminal étroit |
