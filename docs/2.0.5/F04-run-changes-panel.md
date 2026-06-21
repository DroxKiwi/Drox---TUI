# F04 — Panneau changements + diff

**Statut** : spec · **Ligne** : 2.0.5 M1  
**Consommateurs** : colonne droite TUI, multi-diff IDE

---

## Objectif

Lister les **fichiers modifiés pendant le run** (live, pas seulement en fin de run) ; afficher le diff du fichier sélectionné ; navigation clic / `j`/`k`.

---

## Comportement

- Mise à jour à chaque `ToolFinish` `file_write` / `file_edit` / `notebook_edit`.
- Sélection par défaut : **dernier fichier modifié**.
- Clic autre entrée → diff complet (`LinesViewerState` / renderer IDE).
- Beat ID (F02) affiché sur chaque ligne.

---

## Modèle de données

```json
{
  "changes": [
    {
      "path": "RULES.md",
      "kind": "write",
      "beat_id": "A14",
      "patch_unified": "--- a/RULES.md\n+++ b/RULES.md\n...",
      "selected": true
    }
  ]
}
```

Réutilise `RunFileChange` existant + champ `beat_id` optionnel.

---

## Réutilisation 2.0.4

| Existant | Fichier |
|---|---|
| Collecte changements | `view/tool_output.rs` |
| Rendu diff | `view/diff_render.rs`, `lines_viewer.rs` |
| Bandeau fin de run | `app/run.rs` — devient **redundant** si panneau live ; garder comme fallback |

---

## Diff inline fil

Sous-feature **optionnelle** (M3) : patch tronqué dans le fil **en plus** du panneau — pas le cœur de F04.

---

## Port IDE

- Liste dans sidebar + diff editor natif VS Code.
- Même liste `changes[]` ; patch → API diff IDE.
