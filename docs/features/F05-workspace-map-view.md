# F05 — Carte workspace lisible

**Statut** : spec · **Ligne** : 2.0.5 M2  
**Consommateurs** : colonne centrale TUI, tree view IDE

---

## Objectif

Vue **pratique à lire** (pas mermaid) de la connaissance workspace — alimentée par **F03 Context manifest** + trajectoire `WorkspaceMapStore`.

---

## Rendu TUI (v1)

```text
[Carte · tour 7 · ~42k/128k tok]
● src/lib.rs          A12  en contexte
○ docs/old.md         A5   évincé (snip)
◇ tests/              —    structure
▲ RULES.md            A14  modifié ce run
```

- Arbre indenté, scroll vers nœud actif (dernier beat).
- Légende compacte en en-tête pane.
- **Pas** de layout graphique auto en v1.

---

## Interaction

| Action | Effet |
|---|---|
| Clic nœud | Focus + surbrillance beat (F02) |
| Clic beat ID | Sync fil + panneau changements |
| Toggle pane | F01 |

---

## Données

Merge de :

1. `ContextManifest.nodes` (état contextuel — **prioritaire**)
2. `WorkspaceMapStore` (pivots, children — structure)

Sortie : `MapViewSnapshot` dans `drox-observe`.

---

## Port IDE

- `TreeView` explorateur enrichi (badges état + beat).
- Pas de rendu ASCII — même sémantique, widgets natifs.

---

## Hors scope v1

- Graphe mermaid-like
- Layout force-directed
- Édition depuis la carte
