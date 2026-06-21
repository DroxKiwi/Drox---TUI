# F01 — Multi-pane shell

**Statut** : spec · **Ligne** : 2.0.5 M0  
**Consommateurs** : TUI ratatui, IDE webview

---

## Objectif

Permettre **plusieurs vues simultanées** (fil, carte, changements) avec **masquage / affichage** par panneau — sans imposer un triptyque permanent sur terminaux étroits.

---

## Comportement

| Action | Effet |
|---|---|
| Toggle panneau carte | Affiche / masque colonne centrale |
| Toggle panneau changements | Affiche / masque colonne droite |
| `Tab` | Cycle focus : fil → carte → changements → composer |
| `Esc` | Ferme panneau focus ou restaure layout par défaut |
| Terminal étroit | Fallback 1–2 panes ; prefs mémorisées |

**Composer** : toujours pleine largeur, jamais scindé.

---

## Modèle de données (`PaneLayout`)

```json
{
  "panes": {
    "feed": { "visible": true, "width_pct": 40 },
    "map": { "visible": true, "width_pct": 30 },
    "changes": { "visible": true, "width_pct": 30 }
  },
  "focus": "feed",
  "preset": "triptych | dual_map | dual_changes | feed_only"
}
```

Persisté dans `tui-preferences.json` (TUI) ou settings IDE.

---

## Port IDE

- Équivalent : `AgentsWindow` avec 3 `ViewPane` redimensionnables.
- Même struct `PaneLayout` sérialisée côté extension.

---

## Hors scope F01

- Contenu des panes (→ F03, F04, F05)
- Beat ID (→ F02)
