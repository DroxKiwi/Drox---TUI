# F02 — Beat ID & corrélation couleur

**Statut** : spec · **Ligne** : 2.0.5 M0–M1  
**Consommateurs** : fil TUI, carte, panneau diff, timeline IDE

---

## Objectif

Assigner un identifiant **mécanique** (`A1`, `A2`, …) à chaque **beat observable** du run, avec une **couleur stable** reprise sur les trois panes — réduire la charge cognitive sans demander au LLM d’émettre les IDs.

---

## Beat observable (qui reçoit un ID)

| Événement source | Exemple beat |
|---|---|
| `PhaseEnter` | `A1` — `[phase: reading]` |
| `ToolStart` / `ToolFinish` | `A2` — `file_read src/lib.rs` |
| `ContextSnip` / `ContextCompacted` | `A3` — compaction (meta) |
| Écriture fichier | `A4` — `file_write RULES.md` |

**Non** : tokens texte assistant libres, marqueurs `[phase: …]` inventés par le modèle hors parseur.

---

## Modèle de données

```json
{
  "beat_id": "A12",
  "color_index": 3,
  "kind": "tool_finish",
  "phase": "acting",
  "tool_name": "file_read",
  "paths": ["src/lib.rs"],
  "timestamp_ms": 1710000000123
}
```

- `color_index` → palette theme (`beat_0` … `beat_7`, cycle).
- Mapping beat → paths : alimente surbrillance carte (F05) et liste changements (F04).

---

## Corrélation tryptique

```text
Fil        : ▸ A12 file_read src/lib.rs
Carte      : src/lib.rs  [A12]  (couleur beat_3)
Changements: (si modifié plus tard) lien « lu en A12 »
```

Clic sur `A12` dans un pane → focus + scroll vers entité liée dans les autres panes.

---

## Couche observe

- **`BeatRegistry`** dans `drox-observe` : assigne IDs, expose lookup.
- Alimenté par adaptateur moteur (écoute `AgentEvent` legacy).
- **Aucune modification** du protocole prompt LLM.

---

## Port IDE

- Pastille `A12` dans timeline chat + gutter editor coloré.
- Même `BeatRegistry` via stream RPC `observe/event`.
