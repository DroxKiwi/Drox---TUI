# Ligne produit `2.0.4` — Drox TUI · Diff visuel (clôturée)

**Version produit** : `2.0.4`  
**Branche Git** : `2.0.4` → mergée dans `main`  
**Release OR** : [v2.0.4](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.4)  
**Prédécesseur** : [`2.0.3`](../2.0.3/README.md)

---

## Objectifs livrés

| Thème | Statut |
|---|---|
| **Diff visuel TUI** (M1–M4) | ✅ |
| **Correctifs `/update`** (header, palette) | ✅ |
| **Fix réponses double** | ✅ |

---

## Reporté

| Thème | Ligne |
|---|---|
| Code signing Windows + GPG Linux | [`2.0.6`](../2.0.6/README.md) |
| Diff inline fil + split pane | [`2.0.5`](../2.0.5/README.md) |
| Animation splash IDE | [`animation-start`](../animation-start/README.md) |

---

## Documents

| Document | Rôle |
|---|---|
| [PLAN-VISUAL-DIFF.md](PLAN-VISUAL-DIFF.md) | Jalons M1–M4 |
| [RELEASE_NOTES.md](RELEASE_NOTES.md) | Notes release OR |
| [BUG-DUPLICATE-RESPONSES.md](BUG-DUPLICATE-RESPONSES.md) | Correctif streaming |
| [PLAN-CODE-SIGNING.md](PLAN-CODE-SIGNING.md) | Archivé → voir 2.0.6 |
| [CHECKLIST.md](CHECKLIST.md) | Clôture |

---

## Références code

| Zone | Fichier |
|---|---|
| `/diff` | `engine/diff_cmd.rs` |
| Viewer diff | `view/lines_viewer.rs`, `view/diff_render.rs` |
| Fil agent | `view/tool_output.rs` |
| `/update` | `slash/update.rs`, `ui/layout.rs` |
